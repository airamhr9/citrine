use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use citrine_core::application::ApplicationBuilder;
use citrine_core::jsonwebtoken::Algorithm;
use citrine_core::request::Request;
use citrine_core::response::Response;
use citrine_core::security::{
    Authenticator, JWTConfiguration, MethodMatcher, SecurityAction, SecurityConfiguration,
};
use citrine_core::{
    self, tera, tokio, DefaultErrorResponseBody, Method, RequestError, Router, ServerError,
    StatusCode,
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

    // This is a dummy JWT secret key for testing purposes. You should generate one and use it via environment variables
    let jwt_secret = "NTNv7j0TuYARvmNMmWXo6fKvM4o6nv/aUi9ryX38ZH+L1bkrnD1ObOQ8JAUmHCBq7Iy7otZcyAagBLHVKvvYaIpmMuxmARQ97jUVG16Jkpkp1wXOPsrF9zwew6TpczyHkHgX5EuLg2MeBuiT/qJACs1J0apruOOJCg/gOtkjB4c=";

    ApplicationBuilder::<State>::new()
        .name("Citrine sample application")
        .version("0.0.1")
        .port(8080)
        .interceptor(|request, response| {
            let user = if let Some(claims) = request.auth_result.get_claims() {
                claims
                    .name
                    .clone()
                    .unwrap_or("No user in token".to_string())
            } else {
                "Empty".to_string()
            };

            info!(
                "User: {} | Request: {} {} body: {:?} | Response: {}",
                user,
                request.method,
                request.uri,
                request.get_body_raw(),
                response.status,
            )
        })
        // we serve all of the files under the ./public folder in the base path of our application
        .serve_static_files("/", PathBuf::from("./public"))
        .configure_tera(|mut tera| {
            tera.register_filter("url_encode", url_encode_filter);
            tera
        })
        .security_configuration(
            SecurityConfiguration::new()
                // We protect writes in the /api subdomain but allow reads
                .add_rule(
                    MethodMatcher::Multiple(vec![Method::POST, Method::PUT]),
                    "/api/*",
                    SecurityAction::Authenticate(Authenticator::JWT(JWTConfiguration::new(
                        jwt_secret,
                        Algorithm::HS256,
                    ))),
                )
                // Any other request is allowed. This is the default behaviour if this line is
                // removed, but adding it makes it more explicit what you want to do with with
                // the requests that do not match the rules above
                .add_rule(MethodMatcher::All, "/*", SecurityAction::Allow),
        )
        .router(
            Router::new()
                .add_route(Method::GET, "", base_path_controller)
                .add_router(Router::base_path("/api").add_router(user_router()))
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
            skytable::pool::get(8, Config::new_default("root", "123456789101112131415")).unwrap();

        let mut db = pool.get().unwrap();

        // set up database
        db.query_parse::<bool>(&query!("drop space if exists allow not empty sample"))
            .unwrap();
        db.query_parse::<()>(&query!("create space sample"))
            .unwrap();
        db.query_parse::<()>(&query!(
            "create model sample.users(id: string, username: string, mail: string, profile_picture_url: string)"
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
    pub username: String,
    pub mail: String,
    pub profile_picture_url: String,
}

impl From<CreateUser> for User {
    fn from(value: CreateUser) -> Self {
        User {
            id: value.id,
            username: value.username,
            mail: value.mail,
            profile_picture_url: String::new(),
        }
    }
}

#[derive(skytable::Query, Deserialize, Validate)]
pub struct CreateUser {
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

#[derive(Serialize)]
pub struct UserListResponse {
    users: Vec<User>,
}

/*
 * This is the handler for the / path. In this case we are going to return an HTML template
 * */

fn base_path_controller(state: Arc<State>, _: Request) -> Response {
    let mut db = state.get_db_connection();
    let users = UserListResponse {
        users: find_all_users(&mut db),
    };

    Response::view("index.html", &users).unwrap()
}

/*
 * This is the users REST router, this adds all of the REST endpoints  for the user entity to the
 * application and sets a function handler for each.
 * */

fn user_router() -> Router<State> {
    Router::base_path("/users")
        .add_route(Method::GET, "", find_all_users_controller)
        .add_route(Method::GET, "/:id", find_by_id_controller)
        .add_route(Method::DELETE, "/:id", delete_by_id_controller)
        .add_route(Method::PUT, "/:id", update_user_controler)
        .add_route(Method::POST, "", create_user_controler)
}

/*
 * This are the REST endpoint handlers. They receive the application's state struct and the request
 * as parameters.
 * */

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
    let read_body_res: Result<CreateUser, RequestError> = req.get_body_validated();
    if let Err(e) = read_body_res {
        return e.to_response();
    }

    let user = read_body_res.unwrap();
    let mut db = state.db.get().unwrap();

    if let Err(e) = create(user.into(), &mut db) {
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

    users.to_vec()
}

fn find_by_id(id: &String, db: &mut DbConnection) -> Option<User> {
    let user_res: Result<User, _> =
        db.query_parse(&query!("select * from sample.users where id = ?", id));
    if let Ok(user) = user_res {
        Some(user)
    } else {
        None
    }
}

fn create(user: User, db: &mut DbConnection) -> Result<(), skytable::error::Error> {
    db.query_parse::<()>(&query!("insert into sample.users(?, ?, ?, ?)", user))
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
        "update sample.users set username = ?, mail = ?, profile_picture_url ? where id = ?",
        &req,
        id
    ))
}

// A filter to use in our Tera templates
fn url_encode_filter(
    value: &tera::Value,
    _: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let input = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("Expected a string for url_encode filter"))?;

    let encoded = url::form_urlencoded::byte_serialize(input.as_bytes()).collect();

    Ok(tera::Value::String(encoded))
}

// Mock data to to insert on intialization

lazy_static! {
    static ref USERS: Vec<User> = vec![
        User {
            id: String::from("1"),
            username: String::from("alice123"),
            mail: String::from("alice@example.com"),
            profile_picture_url: String::from("https://i.pravatar.cc/150?u=alice@example.com"),
        },
        User {
            id: String::from("2"),
            username: String::from("bob_the_builder"),
            mail: String::from("bob@builder.com"),
            profile_picture_url: String::new()
        },
        User {
            id: String::from("3"),
            username: String::from("charlie_brown"),
            mail: String::from("charlie@peanuts.com"),
            profile_picture_url: String::from("https://i.pravatar.cc/150?u=charlie@peanuts.com"),
        },
        User {
            id: String::from("4"),
            username: String::from("dave99"),
            mail: String::from("dave99@example.com"),
            profile_picture_url: String::from("https://i.pravatar.cc/150?u=dave99@example.com"),
        },
        User {
            id: String::from("5"),
            username: String::from("eve_adventurer"),
            mail: String::from("eve@adventure.com"),
            profile_picture_url: String::from("https://i.pravatar.cc/150?u=eve@adventure.com"),
        },
        User {
            id: String::from("6"),
            username: String::from("frankie_music"),
            mail: String::from("frankie@music.com"),
            profile_picture_url: String::new()
        },
        User {
            id: String::from("7"),
            username: String::from("grace_hopper"),
            mail: String::from("grace@hopper.com"),
            profile_picture_url: String::from("https://i.pravatar.cc/150?u=grace@hopper.com"),
        },
        User {
            id: String::from("8"),
            username: String::from("hank_punny"),
            mail: String::from("hank@pun.com"),
            profile_picture_url: String::from("https://i.pravatar.cc/150?u=hank@pun.com"),
        },
        User {
            id: String::from("9"),
            username: String::from("ivy_lee"),
            mail: String::from("ivy@leaf.com"),
            profile_picture_url: String::new()
        },
        User {
            id: String::from("10"),
            username: String::from("jack_sparrow"),
            mail: String::from("jack@caribbean.com"),
            profile_picture_url: String::from("https://i.pravatar.cc/150?u=jack@caribbean.com"),
        },
    ];
}
