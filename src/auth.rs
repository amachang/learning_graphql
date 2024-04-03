use anyhow::{bail, Result};
use uuid::Uuid;
use chrono::{Utc, NaiveDateTime};
use serde_json::{to_value, from_value};
use actix_session::Session;
use actix_web::{web, HttpResponse};
use sea_orm::prelude::*;
use sea_orm::ActiveValue::Set;
use webauthn_rs::prelude::{
    Passkey,
    Webauthn,
    CreationChallengeResponse,
    PasskeyRegistration,
    RegisterPublicKeyCredential,
    RequestChallengeResponse,
    PasskeyAuthentication,
    PublicKeyCredential,
};
use futures::future::FutureExt;

use super::{
    Error,
    db,
    entity::{user, passkey},
};


pub async fn start_registration(session: Session, webauthn: web::Data<Webauthn>) -> Result<web::Json<CreationChallengeResponse>, Error> {
    let res = start_registration_anyhow_result(session, webauthn).await?;
    Ok(res)
}

async fn start_registration_anyhow_result(session: Session, webauthn: web::Data<Webauthn>) -> Result<web::Json<CreationChallengeResponse>> {
    session.remove("reg_state");

    let user_id = Uuid::new_v4();
    let username = format!("user-{}", user_id);
    let (ccr, reg_state) = webauthn.start_passkey_registration(user_id, &username, "New User", None)?;

    session.insert("reg_state", (user_id, reg_state))?;
    Ok(web::Json(ccr))
}

pub async fn finish_registration(req: web::Json<RegisterPublicKeyCredential>, session: Session, webauthn: web::Data<Webauthn>) -> Result<HttpResponse, Error> {
    let res = finish_registration_anyhow_result(req, session, webauthn).await?;
    Ok(res)
}

async fn finish_registration_anyhow_result(req: web::Json<RegisterPublicKeyCredential>, session: Session, webauthn: web::Data<Webauthn>) -> Result<HttpResponse> {
    let (user_id, reg_state): (Uuid, PasskeyRegistration) = match session.remove_as("reg_state") {
        None => bail!("No registration state found"),
        Some(Err(str)) => bail!("Invalid registration state: {}", str),
        Some(Ok(val)) => val,
    };

    let passkey = webauthn.finish_passkey_registration(&req, &reg_state)?;

    let conn = db::connect().await?;
    let user = db::transaction(&conn, move |txn| async move {
        let now = Utc::now();
        let now: NaiveDateTime = now.naive_utc();
        let user = user::ActiveModel {
            id: Set(user_id.clone()),
            registered_at: Set(now),
            ..Default::default()
        };
        let user = user.insert(txn).await?;
        let passkey = passkey::ActiveModel {
            user_id: Set(user_id),
            content: Set(to_value(passkey)?),
            ..Default::default()
        };
        passkey.insert(txn).await?;
        Ok(user)
    }.boxed()).await?;

    session.insert("user", user)?;
    Ok(HttpResponse::Ok().finish())
}

pub async fn start_authentication(user_id: web::Json<Uuid>, session: Session, webauthn: web::Data<Webauthn>) -> Result<web::Json<RequestChallengeResponse>, Error> {
    let res = start_authentication_anyhow_result(user_id, session, webauthn).await?;
    Ok(res)
}

async fn start_authentication_anyhow_result(user_id: web::Json<Uuid>, session: Session, webauthn: web::Data<Webauthn>) -> Result<web::Json<RequestChallengeResponse>> {
    session.remove("auth_state");
    let user_id = user_id.into_inner();

    let conn = db::connect().await?;
    let passkeys = db::transaction(&conn, move |txn| async move {
        let passkeys = passkey::Entity::find()
            .filter(passkey::Column::UserId.eq(user_id))
            .all(txn)
            .await?;
        Ok(passkeys)
    }.boxed()).await?;

    let passkeys: Vec<Passkey> = passkeys.into_iter().map(|passkey| {
        let passkey = from_value(passkey.content)?;
        Ok(passkey)
    }).collect::<Result<Vec<_>>>()?;

    let (rcr, auth_state) = webauthn.start_passkey_authentication(&passkeys)?;

    session.insert("auth_state", (user_id, auth_state))?;

    Ok(web::Json(rcr))
}

pub async fn finish_authentication(req: web::Json<PublicKeyCredential>, session: Session, webauthn: web::Data<Webauthn>) -> Result<HttpResponse, Error> {
    let res = finish_authentication_anyhow_result(req, session, webauthn).await?;
    Ok(res)
}

async fn finish_authentication_anyhow_result(req: web::Json<PublicKeyCredential>, session: Session, webauthn: web::Data<Webauthn>) -> Result<HttpResponse> {
    let (user_id, auth_state): (Uuid, PasskeyAuthentication) = match session.remove_as("auth_state") {
        None => bail!("No authentication state found"),
        Some(Err(str)) => bail!("Invalid authentication state: {}", str),
        Some(Ok(val)) => val,
    };

    let auth_result = webauthn.finish_passkey_authentication(&req, &auth_state)?;
    let user_verified = auth_result.user_verified();

    let conn = db::connect().await?;
    db::transaction(&conn, move |txn| async move {
        let passkeys = passkey::Entity::find()
            .filter(passkey::Column::UserId.eq(user_id))
            .all(txn)
            .await?;
        for passkey in passkeys {
            let mut passkey_content: Passkey = from_value(passkey.content.clone())?;
            if passkey_content.update_credential(&auth_result) == Some(true) {
                let mut passkey: passkey::ActiveModel = passkey.into();
                passkey.content = Set(to_value(passkey_content)?);
                passkey.update(txn).await?;
            }
        }
        Ok(())
    }.boxed()).await?;

    if !user_verified {
        bail!("Authentication failed");
    }

    let user = db::transaction(&conn, move |txn| async move {
        let user = user::Entity::find_by_id(user_id).one(txn).await?;
        Ok(user)
    }.boxed()).await?;

    session.insert("user", user)?;
    Ok(HttpResponse::Ok().finish())
}

