use std::sync::Arc;

use citrine_core::application::ApplicationBuilder;
use citrine_core::request::Request;
use citrine_core::response::Response;
use citrine_core::{
    self, tokio, DefaultErrorResponseBody, Method, RequestError, Router, ServerError, StatusCode,
};
use validator::Validate;

use log::info;
use r2d2::PooledConnection;
use serde::{Deserialize, Serialize};
use skytable::pool::ConnectionMgrTcp;
use skytable::response::Rows;
use skytable::{query, Config};

#[macro_use]
extern crate lazy_static;

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    env_logger::init();

    ApplicationBuilder::<State>::new()
        .name("Citrine sample application")
        .version("0.0.1")
        .port(8080)
        .interceptor(|request, response| {
            info!(
                "Request: {} {} body: {:?}. Response: {}",
                request.method,
                request.uri,
                request.get_body_raw(),
                response.status,
            )
        })
        .add_routes(
            Router::new()
                .add_route(Method::GET, "", base_path_controller)
                .add_router(Router::base_path("/api").add_router(user_router())),
        )
        .start()
        .await
}

/*
 * This is the state struct, which allows access to shared information in the request handlers,
 * like DB connections. It should ideally be immutable, in order to avoid having to wrap it with
 * some Lock or Mutex and avoid bottlenecks. That's why in this example we use a DB Connection pool
 * instead of a single connection.
 *
 * All State functions must implement the Default trait. Here, we use it to intialize the database
 * connection pool, create the model and insert some mock data.
 * */

pub struct State {
    db: r2d2::Pool<ConnectionMgrTcp>,
}

type DbConnection = PooledConnection<ConnectionMgrTcp>;

impl State {
    fn get_db_connection(&self) -> DbConnection {
        self.db.get().unwrap()
    }
}

impl Default for State {
    fn default() -> Self {
        // create connection pool
        let pool =
            skytable::pool::get(32, Config::new_default("root", "123456789101112131415")).unwrap();

        let mut db = pool.get().unwrap();

        // set up database
        db.query_parse::<bool>(&query!("drop space if exists allow not empty sample"))
            .unwrap();
        db.query_parse::<()>(&query!("create space sample"))
            .unwrap();
        db.query_parse::<()>(&query!(
            "create model sample.users(id: string, username: string, mail: string)"
        ))
        .unwrap();

        for user in USERS.iter() {
            create(user.clone(), &mut db).unwrap();
        }

        State { db: pool }
    }
}

/*
 * This is the application domain, that contains an entity User and an Update User request struct.
 *
 * To be able to receive them in a REST endpoint, they must derive serde::Deserialize, and
 * serde::Serialize to return them as a response.
 *
 * They also need to derive skytable::Query to use them in DB Queries and skytable::Response to be
 * returned from them, but this will vary based on your choice of persistence.
 * */

#[derive(skytable::Query, skytable::Response, Clone, Serialize, Deserialize, Validate)]
pub struct User {
    pub id: String,
    #[validate(length(min = 1))]
    pub username: String,
    #[validate(email)]
    pub mail: String,
}

#[derive(skytable::Query, Deserialize, Validate)]
pub struct UpdateUser {
    #[validate(length(min = 1))]
    pub username: String,
    #[validate(email)]
    pub mail: String,
}

/*
 * This is the router, this adds all the REST endpoints to the application and sets a function
 * handler for each.
 * */

fn user_router() -> Router<State> {
    return Router::base_path("/users")
        .add_route(Method::GET, "", find_all_users_controller)
        .add_route(Method::GET, "/:id", find_by_id_controller)
        .add_route(Method::DELETE, "/:id", delete_by_id_controller)
        .add_route(Method::PUT, "/:id", update_user_controler)
        .add_route(Method::POST, "", create_user_controler);
}

/*
 * This are the REST endpoint handlers. They receive the application's state struct and the request
 * as parameters.
 * */

fn base_path_controller(_: Arc<State>, req: Request) -> Response {
    let body = req.get_body_raw().clone();

    Response::new(StatusCode::OK).body(format!(
        "This is the base path. Echo: {}",
        body.unwrap_or("".to_string())
    ))
}

fn find_all_users_controller(state: Arc<State>, _: Request) -> Response {
    let mut db = state.get_db_connection();

    let users = find_all_users(&mut db);

    Response::new(StatusCode::OK).json(users)
}

fn find_by_id_controller(state: Arc<State>, req: Request) -> Response {
    let path_variables = req.path_variables;
    let id = path_variables.get("id").unwrap();

    let opt_user = find_by_id(id, &mut state.get_db_connection());
    if let Some(user) = opt_user {
        Response::new(StatusCode::OK).json(user)
    } else {
        Response::new(StatusCode::NOT_FOUND)
    }
}

fn delete_by_id_controller(state: Arc<State>, req: Request) -> Response {
    let path_variables = req.path_variables;
    let id = path_variables.get("id").unwrap();

    let mut db = state.db.get().unwrap();

    if let Err(e) = delete(id, &mut db) {
        Response::new(StatusCode::NO_CONTENT).json(DefaultErrorResponseBody::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        ))
    } else {
        Response::new(StatusCode::NO_CONTENT)
    }
}

fn create_user_controler(state: Arc<State>, req: Request) -> Response {
    let read_body_res: Result<User, RequestError> = req.get_body_validated();
    if let Err(e) = read_body_res {
        return e.to_response();
    }

    let user = read_body_res.unwrap();
    let mut db = state.db.get().unwrap();

    if let Err(e) = create(user, &mut db) {
        Response::new(StatusCode::INTERNAL_SERVER_ERROR).json(DefaultErrorResponseBody::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        ))
    } else {
        Response::new(StatusCode::NO_CONTENT)
    }
}

fn update_user_controler(state: Arc<State>, req: Request) -> Response {
    let read_body_res: Result<UpdateUser, RequestError> = req.get_body_validated();
    if let Err(e) = read_body_res {
        return e.to_response();
    }

    let user = read_body_res.unwrap();
    let id = req.path_variables.get("id").unwrap();

    let mut db = state.db.get().unwrap();

    if let Err(e) = update(id, user, &mut db) {
        Response::new(StatusCode::INTERNAL_SERVER_ERROR).json(DefaultErrorResponseBody::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        ))
    } else {
        Response::new(StatusCode::NO_CONTENT)
    }
}

/*
 * This are the "service layer" functions and contain the business logic.
 * */

fn find_all_users(db: &mut DbConnection) -> Vec<User> {
    let users: Rows<User> = db
        .query_parse(&query!("select all * from sample.users limit ?", 1000u64))
        .unwrap();

    return users.to_vec();
}

fn find_by_id(id: &String, db: &mut DbConnection) -> Option<User> {
    let user: Result<User, _> =
        db.query_parse(&query!("select * from sample.users where id = ?", id));
    if user.is_err() {
        None
    } else {
        Some(user.unwrap())
    }
}

fn create(user: User, db: &mut DbConnection) -> Result<(), skytable::error::Error> {
    db.query_parse::<()>(&query!("insert into sample.users(?, ?, ?)", user))
}

fn delete(id: &String, db: &mut DbConnection) -> Result<(), skytable::error::Error> {
    db.query_parse::<()>(&query!("delete from sample.users where id = ?", &id))
}

fn update(
    id: &String,
    req: UpdateUser,
    db: &mut DbConnection,
) -> Result<(), skytable::error::Error> {
    db.query_parse::<()>(&query!(
        "update sample.users set username = ?, mail = ? where id = ?",
        &req,
        id
    ))
}

/*
 * This is just some mock data to to insert on intialization
 * */

lazy_static! {
    static ref USERS: Vec<User> = vec![
        User {
            id: String::from("1"),
            username: String::from("alice123"),
            mail: String::from("alice@example.com"),
        },
        User {
            id: String::from("2"),
            username: String::from("bob_the_builder"),
            mail: String::from("bob@builder.com"),
        },
        User {
            id: String::from("3"),
            username: String::from("charlie_brown"),
            mail: String::from("charlie@peanuts.com"),
        },
        User {
            id: String::from("4"),
            username: String::from("dave99"),
            mail: String::from("dave99@example.com"),
        },
        User {
            id: String::from("5"),
            username: String::from("eve_adventurer"),
            mail: String::from("eve@adventure.com"),
        },
        User {
            id: String::from("6"),
            username: String::from("frankie_music"),
            mail: String::from("frankie@music.com"),
        },
        User {
            id: String::from("7"),
            username: String::from("grace_hopper"),
            mail: String::from("grace@hopper.com"),
        },
        User {
            id: String::from("8"),
            username: String::from("hank_punny"),
            mail: String::from("hank@pun.com"),
        },
        User {
            id: String::from("9"),
            username: String::from("ivy_lee"),
            mail: String::from("ivy@leaf.com"),
        },
        User {
            id: String::from("10"),
            username: String::from("jack_sparrow"),
            mail: String::from("jack@caribbean.com"),
        },
    ];
}
