use std::sync::{Arc, Weak};
use anyhow::{anyhow, Result};
use url::Url;
use sea_orm::prelude::*;
use sea_orm::{
    DatabaseTransaction,
    TransactionTrait,
};
use clap::Parser;
use actix_session::{Session, SessionMiddleware};
use actix_web::{guard, web, App, HttpServer, HttpResponse, cookie, ResponseError, http::StatusCode};
use async_graphql::{extensions, Object, EmptyMutation, EmptySubscription, Schema, Context, http::{playground_source, GraphQLPlaygroundConfig}};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
use webauthn_rs::prelude::WebauthnBuilder;

mod db;
mod auth;
mod session;
mod entity;

use entity::{post, user};
use session::MemorySession;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("an unspecified internal error occurred: {0}")]
    InternalError(#[from] anyhow::Error),
}

impl ResponseError for Error {

    fn status_code(&self) -> StatusCode {
        match &self {
            Self::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).body(self.to_string())
    }

}

#[derive(Debug, Parser)]
struct Args {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Debug, Parser)]
enum SubCommand {
    HttpServer { hostname: String, port: u16 },
}

#[derive(Debug)]
struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn hello(&self) -> &'static str {
        "Hello, graphql!"
    }

    async fn posts(&self, ctx: &Context<'_>) -> Result<Vec<post::Model>> {
        let trx = trx_from_ctx(ctx)?;

        let posts = post::Entity::find().all(trx.as_ref()).await?;
        Ok(posts)
    }

    async fn users(&self, ctx: &Context<'_>) -> Result<Vec<user::Model>> {
        let trx = trx_from_ctx(ctx)?;

        let users = user::Entity::find().all(trx.as_ref()).await?;
        Ok(users)
    }
}

fn trx_from_ctx(ctx: &Context<'_>) -> Result<Arc<DatabaseTransaction>> {
    ctx.data::<Weak<DatabaseTransaction>>().map_err(|err| anyhow!("no transaction: {:?}", err))?
        .upgrade().ok_or_else(|| anyhow!("transaction is already dropped"))
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    match args.subcmd {
        SubCommand::HttpServer { hostname, port } => {
            let hostname_cloned = hostname.clone();
            HttpServer::new(move || {
                let rp_id = &hostname_cloned;
                let rp_origin = Url::parse(&format!("http://{}:{}", hostname_cloned, port)).expect("hostname and port must be valid");
                let webauthn = WebauthnBuilder::new(&rp_id, &rp_origin).expect("correct webauthn origin is prerequisite").build();

                let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
                    .extension(extensions::Logger)
                    .finish();

                App::new()
                    .wrap(
                        // Session middleware, just memory store for now, so key is also temporary
                        SessionMiddleware::builder(MemorySession, cookie::Key::generate())
                            .cookie_name("sess_id".to_string())
                            .cookie_http_only(true)
                            .cookie_secure(false)
                            .build()
                    )
                    .service(
                        web::scope("/auth")
                            .app_data(web::Data::new(webauthn))
                            .service(web::resource("/start_register").guard(guard::Post()).to(auth::start_registration))
                            .service(web::resource("/finish_register").guard(guard::Post()).to(auth::finish_registration))
                            .service(web::resource("/start_auth").guard(guard::Post()).to(auth::start_authentication))
                            .service(web::resource("/finish_auth").guard(guard::Post()).to(auth::finish_authentication))
                    )
                    .service(web::resource("/").guard(guard::Get()).to(hello))
                    .service(
                        web::resource("/graphql")
                            .app_data(web::Data::new(schema))
                            .guard(guard::Post()).to(
                                handle_graphql)
                    )
                    .service(web::resource("/playground").guard(guard::Get()).to(graphql_playgound))
            }).bind((hostname, port))?.run().await?;
        },
    }

    Ok(())
}

async fn hello() -> &'static str {
    "Hello, world!"
}

async fn graphql_playgound() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

async fn handle_graphql(session: Session, schema: web::Data<Schema<QueryRoot, EmptyMutation, EmptySubscription>>, req: GraphQLRequest) -> Result<GraphQLResponse, Error> {
    let res = handle_graphql_anyhow_result(session, schema, req).await?;
    Ok(res)
}

async fn handle_graphql_anyhow_result(session: Session, schema: web::Data<Schema<QueryRoot, EmptyMutation, EmptySubscription>>, req: GraphQLRequest) -> Result<GraphQLResponse> {
    let req = req.into_inner();

    let req = if let Some(user) = session.get::<user::Model>("user")? {
        req.data(user.clone())
    } else {
        req
    };

    let conn = db::connect().await?;

    let trx = conn.begin().await?;
    let trx = Arc::new(trx);
    let res = schema.execute(
        req.data(Arc::downgrade(&trx)),
    ).await;
    let trx = Arc::try_unwrap(trx).expect("only one reference to the transaction should exist");
    if res.is_err() {
        let _ = trx.rollback().await;
        return Ok(res.into());
    }
    trx.commit().await?;

    Ok(res.into())
}

