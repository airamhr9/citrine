
use citrine_core::request::Request;
use citrine_core::response::Response;
use citrine_core::{self, tokio, Method, RequestError, Router, ServerError, StatusCode};
use citrine_core::application::ApplicationBuilder;

use log::info;
use serde::{Deserialize, Serialize};

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
        .add_routes(user_router())
        .add_routes(Router::new().add_route(Method::GET, "", base_path_controller))
        .start()
        .await
}

fn user_router() -> Router<State> {
    return Router::base_path("/users")
        .add_route(Method::GET, "/all", find_all_users_controller)
        .add_route(Method::GET, "/:id", find_by_id_controller)
        .add_route(Method::DELETE, "/:id", delete_by_id_controller)
        .add_route(Method::POST, "", create_user_controler)

}

fn base_path_controller(state: Arc<State>, req: Request) -> Response {
    Response::new(StatusCode::OK).body("This is the base path jeje".to_string())
}

fn find_all_users_controller(state: Arc<State>, req: Request) -> Response {
    let users = find_all_users();

    Response::new(StatusCode::OK).json(users)
}

fn find_by_id_controller(state: Arc<State>, req: Request) -> Response {
    let path_variables = req.path_variables;
    let id = path_variables.get("id").unwrap();

    let opt_user = find_by_id(id);
    if let Some(user) = opt_user {
        Response::new(StatusCode::OK).json(user)
    } else {
        Response::new(StatusCode::NOT_FOUND)
    }
} 

fn delete_by_id_controller(state: Arc<State>, req: Request) -> Response {
    let path_variables = req.path_variables;
    let id = path_variables.get("id").unwrap();

    delete(id);
    Response::new(StatusCode::NO_CONTENT)
} 



fn create_user_controler(state: Arc<State>, req: Request) -> Response {
    let read_body_res: Result<Option<User>, RequestError> = req.get_body(true);
    if let Err(e) = read_body_res {
        return e.to_response();
    }

    let user = read_body_res.unwrap().unwrap();

    create(user);

    Response::new(StatusCode::NO_CONTENT)
}

fn find_all_users() -> Vec<User> {
    return USERS.to_vec(); 
}

fn find_by_id(id: &String) -> Option<User> {
    for user in USERS.iter() {
        if user.id == *id {
            return Some(user.to_owned());
        }
    } 
    None
}

fn create(user: User) {
    USERS.to_vec().push(user);
}

fn delete(id: &String) {
    let mut pos: i32 = -1;
    for (i, user) in USERS.iter().enumerate() {
        if user.id == *id {
            pos = i as i32;
            break;
        }
    }
    if pos != -1 {
        USERS.to_vec().remove(pos as usize);
    }
}
fn update(id: String, req: User) {
    for user in USERS.to_vec().iter_mut() {
        if user.id == id {
            user.username = req.username;
            user.mail = req.mail;
            break;
        }
    }
}

pub struct State {
    pub counter: i32,
}

impl Default for State {
    fn default() -> Self {
        State { counter: 0 }
    }
}

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

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub mail: String,
}
