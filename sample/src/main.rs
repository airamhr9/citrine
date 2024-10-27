use std::collections::HashMap;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

use citrine_core::application::Application;
use citrine_core::jsonwebtoken::Algorithm;
use citrine_core::request::Request;
use citrine_core::response::Response;
use citrine_core::security::{
    Authenticator, JWTConfiguration, MethodMatcher, SecurityAction, SecurityConfiguration,
};
use citrine_core::static_file_server::StaticFileServer;
use citrine_core::{
    self, tera, tokio, DefaultErrorResponseBody, Method, RequestError, Router, ServerError,
    StatusCode,
};
use mock_data::get_mock_users;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension};
use serde_json::json;
use validator::Validate;

use log::{debug, info};
use r2d2::PooledConnection;
use serde::{Deserialize, Serialize};

mod mock_data;

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    env_logger::init();

    // This is a dummy JWT secret key for testing purposes. You should generate one and use it via environment variables
    let jwt_secret = "NTNv7j0TuYARvmNMmWXo6fKvM4o6nv/aUi9ryX38ZH+L1bkrnD1ObOQ8JAUmHCBq7Iy7otZcyAagBLHVKvvYaIpmMuxmARQ97jUVG16Jkpkp1wXOPsrF9zwew6TpczyHkHgX5EuLg2MeBuiT/qJACs1J0apruOOJCg/gOtkjB4c=";

    Application::<Context>::builder()
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
        // We serve all of the files under the ./public folder in the base path of our
        // application and all the files under ./static_views in the path /static
        .serve_static_files(
            StaticFileServer::new()
                .serve_folder("/", PathBuf::from("./public"))
                .serve_folder("/static", PathBuf::from("./static_views")),
        )
        .configure_tera(|mut tera| {
            tera.register_filter("url_encode", url_encode_filter);
            tera
        })
        .security_configuration(
            SecurityConfiguration::new()
                // We protect writes in the /api subdomain but allow reads
                .add_rule(
                    MethodMatcher::Multiple(vec![Method::POST, Method::PUT, Method::DELETE]),
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
                .add_router(Router::base_path("/api").add_router(user_router())),
        )
        .start()
        .await
}

/*
 * This is the context struct, which allows access to shared information in the request handlers,
 * like DB connections. It should ideally be immutable, in order to avoid having to wrap it with
 * some Lock or Mutex and avoid bottlenecks. That's why in this example we use a DB Connection pool
 * instead of a single connection.
 *
 * All Context functions must implement the Default trait. Here, we use it to intialize the database
 * connection pool, create the model and insert some mock data.
 * */

type DbConnection = PooledConnection<SqliteConnectionManager>;
type DbPool = r2d2::Pool<SqliteConnectionManager>;

pub struct Context {
    db: DbPool,
}

impl Context {
    fn get_db_connection(&self) -> DbConnection {
        self.db.get().unwrap()
    }
}

impl Default for Context {
    fn default() -> Self {
        let manager = SqliteConnectionManager::memory();

        let pool = r2d2::Pool::builder().build(manager).unwrap();

        let mut db = pool.get().unwrap();

        match db.execute(&mock_data::get_database_creation_query(), ()) {
            Ok(_) => debug!("In memory database succesfully created"),
            Err(e) => panic!("Error creating in memory database {}", e),
        }

        for user in get_mock_users().iter() {
            create(user.clone(), &mut db).unwrap();
        }

        Context { db: pool }
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

#[derive(Clone, Serialize, Deserialize, Validate)]
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

#[derive(Deserialize, Validate)]
pub struct CreateUser {
    pub id: String,
    #[validate(length(min = 1))]
    pub username: String,
    #[validate(email)]
    pub mail: String,
}

#[derive(Deserialize, Validate)]
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

#[derive(Debug)]
struct SampleError {
    message: String,
    cause: Option<Box<dyn std::error::Error>>,
}

impl SampleError {
    fn new<E>(message: &str, cause: E) -> Self
    where
        E: std::error::Error + 'static,
    {
        SampleError {
            message: message.to_string(),
            cause: Some(Box::new(cause)),
        }
    }
}
impl From<r2d2_sqlite::rusqlite::Error> for SampleError {
    fn from(value: r2d2_sqlite::rusqlite::Error) -> Self {
        SampleError::new("Error interacting with the database", value)
    }
}
impl Display for SampleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(cause) = &self.cause {
            write!(f, "SampleError: {}, caused by: {}", self.message, cause)?
        } else {
            write!(f, "SampleError: {}", self.message)?
        }
        Ok(())
    }
}

/*
 * This is the handler for the / path. In this case we are going to return an HTML template
 * */

fn base_path_controller(context: Arc<Context>, _: Request) -> Response {
    let mut db = context.get_db_connection();
    let users_res = find_all_users(&mut db);
    if users_res.is_err() {
        return Response::view("error.html", &json!({})).unwrap();
    }
    let users = UserListResponse {
        users: users_res.unwrap(),
    };

    Response::view("index.html", &users).unwrap()
}

/*
 * This is the users REST router, this adds all of the REST endpoints  for the user entity to the
 * application and sets a function handler for each.
 * */

fn user_router() -> Router<Context> {
    Router::base_path("/users")
        .add_route(Method::GET, "", find_all_users_controller)
        .add_route(Method::GET, "/:id", find_by_id_controller)
        .add_route(Method::DELETE, "/:id", delete_by_id_controller)
        .add_route(Method::PUT, "/:id", update_user_controler)
        .add_route(Method::POST, "", create_user_controler)
}

/*
 * This are the REST endpoint handlers. They receive the application's context struct and the request
 * as parameters.
 * */

fn find_all_users_controller(context: Arc<Context>, _: Request) -> Response {
    let mut db = context.get_db_connection();

    let users_res = find_all_users(&mut db);
    if let Err(e) = users_res {
        return Response::new(StatusCode::INTERNAL_SERVER_ERROR).json(
            DefaultErrorResponseBody::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        );
    }

    Response::new(StatusCode::OK).json(users_res.unwrap())
}

fn find_by_id_controller(context: Arc<Context>, req: Request) -> Response {
    let path_variables = req.path_variables;
    let id = path_variables.get("id").unwrap();

    let user_res = find_by_id(id, &mut context.get_db_connection());
    if let Err(e) = user_res {
        return Response::new(StatusCode::INTERNAL_SERVER_ERROR).json(
            DefaultErrorResponseBody::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        );
    }
    let opt_user = user_res.unwrap();
    if let Some(user) = opt_user {
        Response::new(StatusCode::OK).json(user)
    } else {
        Response::new(StatusCode::NOT_FOUND)
    }
}

fn delete_by_id_controller(context: Arc<Context>, req: Request) -> Response {
    let path_variables = req.path_variables;
    let id = path_variables.get("id").unwrap();

    let mut db = context.db.get().unwrap();

    if let Err(e) = delete(id, &mut db) {
        Response::new(StatusCode::NO_CONTENT).json(DefaultErrorResponseBody::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        ))
    } else {
        Response::new(StatusCode::NO_CONTENT)
    }
}

fn create_user_controler(context: Arc<Context>, req: Request) -> Response {
    let read_body_res: Result<CreateUser, RequestError> = req.get_body_validated();
    if let Err(e) = read_body_res {
        return e.to_response();
    }

    let user = read_body_res.unwrap();
    let mut db = context.db.get().unwrap();

    if let Err(e) = create(user.into(), &mut db) {
        Response::new(StatusCode::INTERNAL_SERVER_ERROR).json(DefaultErrorResponseBody::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        ))
    } else {
        Response::new(StatusCode::NO_CONTENT)
    }
}

fn update_user_controler(context: Arc<Context>, req: Request) -> Response {
    let read_body_res: Result<UpdateUser, RequestError> = req.get_body_validated();
    if let Err(e) = read_body_res {
        return e.to_response();
    }

    let user = read_body_res.unwrap();
    let id = req.path_variables.get("id").unwrap();

    let mut db = context.db.get().unwrap();

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

fn find_all_users(db: &mut DbConnection) -> Result<Vec<User>, SampleError> {
    let mut stmt = db.prepare("SELECT id, username, mail, profile_picture_url from Users")?;

    let users = stmt
        .query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                mail: row.get(2)?,
                profile_picture_url: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<User>, _>>()?; // Collect rows into a Vec<User>

    Ok(users)
}

fn find_by_id(id: &String, db: &mut DbConnection) -> Result<Option<User>, SampleError> {
    let mut stmt =
        db.prepare("SELECT id, username, mail, profile_picture_url from Users where id = ?1")?;

    let user = stmt
        .query_row(params![id], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                mail: row.get(2)?,
                profile_picture_url: row.get(3)?,
            })
        })
        .optional()?;

    Ok(user)
}

fn create(user: User, db: &mut DbConnection) -> Result<(), SampleError> {
    let res = db.execute(
        "INSERT INTO Users (id, username, mail, profile_picture_url) VALUES (?1, ?2, ?3, ?4)",
        (user.id, user.username, user.mail, user.profile_picture_url),
    );
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(SampleError::new("Error creating user", e)),
    }
}

fn delete(id: &String, db: &mut DbConnection) -> Result<(), SampleError> {
    let res = db.execute("DELETE FROM Users where id = ?1", params![id]);
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(SampleError::new("Error deleting user", e)),
    }
}

fn update(id: &String, req: UpdateUser, db: &mut DbConnection) -> Result<(), SampleError> {
    let res = db.execute(
        "UPDATE Users set username = ?1, mail = ?2 WHERE id = ?4",
        (req.username, req.mail, id),
    );

    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(SampleError::new("Error updating user", e)),
    }
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
