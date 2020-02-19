extern crate actix_web;

pub mod validator;

use crate::validator::{Address, Index, Signature, Value};
use actix_web::{error, middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct VoteRequest {
    user: Address,
    validator: Address,
    amount: Value,
    signature: Signature,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserWithdrawRequest {
    user: Address,
    validator: Address,
    index: Index,
    amount: Value,
    signature: Signature,
}

#[derive(Debug, Serialize, Deserialize)]
struct OwnerWithdrawRequest {
    owner: Address,
    validator: Address,
    signature: Signature,
}

// To vote user must provide:
// - her wallet address
// - validator address
// - support amount
// - signature for: her wallet address, validator address, support amount
async fn vote(item: web::Json<VoteRequest>, req: HttpRequest) -> Result<HttpResponse, Error> {
    // TODO
}

// To withdraw amount and rewards for it the user must provide:
// - validator address
// - her wallet address
// - reward index when that amount has been supported
// - amount to process
// - signature for fields: validator address, user address, index, amount.
async fn user_withdraw_amount_with_reward(
    item: web::Json<UserWithdrawRequest>,
    req: HttpRequest,
) -> Result<HttpResponse, Error> {
    // TODO
}

// To withdraw rewards the validator owner must provide:
// - validator address
// - supposed amount
// - signature for: validator address, her wallet address, supposed amount
async fn owner_withdraw_reward(
    item: web::Json<OwnerWithdrawRequest>,
    req: HttpRequest,
) -> Result<HttpResponse, Error> {
    // TODO
}

// Returns all support indexes for user
async fn get_support_indexes(
    item: web::Json<OwnerWithdrawRequest>,
    req: HttpRequest,
) -> Result<HttpResponse, Error> {
    // TODO
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .service(
                web::resource("/vote")
                    .data(web::JsonConfig::default().limit(1024))
                    .route(web::post().to(vote)),
            )
            .service(
                web::resource("/user_withdraw_amount_with_reward")
                    .data(web::JsonConfig::default().limit(1024))
                    .route(web::post().to(user_withdraw_amount_with_reward)),
            )
            .service(
                web::resource("/owner_withdraw_reward")
                    .data(web::JsonConfig::default().limit(1024))
                    .route(web::post().to(owner_withdraw_reward)),
            )
            .service(
                web::resource("/get_support_indexes")
                    .data(web::JsonConfig::default().limit(1024))
                    .route(web::get().to(owner_withdraw_reward)),
            )
        // TODO: all handlers
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
