use std::collections::HashMap;
use anyhow::Result;
use sea_orm::{
    Database,
    ConnectOptions,
    DatabaseTransaction,
    TransactionTrait,
    ActiveModelTrait,
    ActiveValue::Set,
};
use clap::Parser;
use actix_web::{guard, web, App, HttpServer, HttpResponse};
use async_graphql::{extensions, Object, EmptyMutation, EmptySubscription, Schema, http::{playground_source, GraphQLPlaygroundConfig}};
use async_graphql_actix_web::GraphQL;

mod entity;

use entity::post;
use entity::author;

#[derive(Debug, Parser)]
struct Args {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Debug, Parser)]
enum SubCommand {
    PrepareDummyData,
    HttpServer { port: u16 },
}

#[derive(Debug)]
struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn hello(&self) -> &'static str {
        "Hello, graphql!"
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    match args.subcmd {
        SubCommand::PrepareDummyData => {
            let opt = ConnectOptions::new("sqlite:db/main.db");
            let conn = Database::connect(opt).await?;

            let trx = conn.begin().await?;
            match prepare_dummy_data(&trx).await {
                Ok(_) => trx.commit().await?,
                Err(e) => {
                    trx.rollback().await?;
                    return Err(e);
                }
            }
        },
        SubCommand::HttpServer { port } => {
            HttpServer::new(|| {
                let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
                    .extension(extensions::Logger)
                    .finish();

                App::new()
                    .service(web::resource("/").guard(guard::Get()).to(hello))
                    .service(web::resource("/graphql").guard(guard::Post()).to(GraphQL::new(schema)))
                    .service(web::resource("/playground").guard(guard::Get()).to(graphql_playgound))
            }).bind(("127.0.0.1", port))?.run().await?;
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

async fn prepare_dummy_data(trx: &DatabaseTransaction) -> Result<()> {
    let authors = vec![
        "Alice", "Bob", "Carol", "Dave", "Eve",
    ];
    let posts = vec![
        ("Hello", "Hello, world!", "Alice"),
        ("SeaORM", "SeaORM is an async & dynamic ORM for Rust.", "Alice"),
        ("Rust", "Rust is a systems programming language.", "Bob"),
        ("SQLite", "SQLite is a C-language library that implements a small, fast, self-contained, high-reliability, full-featured, SQL database engine.", "Carol"),
        ("PostgreSQL", "PostgreSQL is a powerful, open source object-relational database system.", "Dave"),
        ("MySQL", "MySQL is an open-source relational database management system.", "Eve"),
    ];

    let mut name_author_map = HashMap::new();
    for author in authors {
        let author = author::ActiveModel {
            name: Set(author.to_owned()),
            ..Default::default()
        };
        let author = author.insert(trx).await?;
        name_author_map.insert(author.name.clone(), author);
    }
    for (title, text, author) in posts {
        let author = name_author_map.get(author).unwrap();
        let post = post::ActiveModel {
            author_id: Set(author.id),
            title: Set(title.to_owned()),
            text: Set(text.to_owned()),
            ..Default::default()
        };
        post.insert(trx).await?;
    }
    Ok(())
}

