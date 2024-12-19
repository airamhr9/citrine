<h1 align="center">
  Citrine

</h1>
<p align="center">
    <a href="/" alt="version">
        <img src="https://img.shields.io/badge/version-0.1.0-blue" />
    </a>
    <a href="https://github.com/airamhr9/citrine/pulse" alt="Activity">
        <img src="https://img.shields.io/github/commit-activity/m/airamhr9/citrine" />
    </a>
</p>


**⚠️ This repository is still in a very early stage of development and is not ready for production. 
All current features and APIs can be completely changed at any given time. ⚠️**


Citrine is a Rust web framework that aims to make Rust web development as easy as possible,
providing all the necessary features to build a complete web application with familiar patterns and APIs.
This is at the moment a personal learning project.

---

## Table of contents

* [Current features](#current-features)
    * [Routing](#routing)
    * [Static file serving](#static-file-serving)
    * [Templates](#templates)
    * [Security](#security)
    * [Multiple Request Types](#multiple-request-types)
    * [Request middleware and response interceptor](#request-middlewares-and-response-interceptor)
    * [Configuration via environment variables](#configuration-via-environment-variables)
    * [Startup Banner](#startup-banner)
* [To Do Before MVP](#to-do-before-mvp)
* [Planned features](#planned-features)

---

## Current features

<div align="center">


</div>

All the examples below are taken directly from the [main.rs](https://github.com/airamhr9/citrine/tree/main/sample/src/main.rs)
file in the sample directory. New features and changes will be displayed in that or new sample projects 
in this repository.

### Routing
#### REST request handling and routing, using [Hyper](https://hyper.rs/) as the HTTP server

The Router struct will contain all the endpoints and handlers for your application. 
Routers can be nested, providing flexibility when designing your API. We can use helpers
for common HTTP methods (GET, POST, PUT, PATCH, DELETE) or pass them as a parameter. The
accepted Content-Type headers are defined for each route, which can be one, multiple or none. 
If you use the helper methods, the accepted Content-Type will be by default only JSON for 
POST, PUT, PATCH and DELETE, and None for GET.

```rust
// Application definition
fn main() -> Result<(), ServerError> {
    Application::<Context>::builder()
        ...
        .router(
            Router::new()
                .get("", base_path_controller)
                .add_router(Router::base_path("/api").add_router(user_router()))
        )
        .start()
        .await
}


// Endpoint handler definition 
fn base_path_controller(context: Arc<Context>, _: Request) -> Response {
 // controller contents
}

// Router definition
fn user_router() -> Router<Context> {
    Router::base_path("/users")
        // Complete route definition with HTTP method and accepted types as a parameters
        .add_route(
            Method::POST,
            "",
            create_user_controler,
            Accepts::Multiple(vec![ContentType::Json, ContentType::FormUrlEncoded]),
        )
        // Helpers for common HTTP methods that only receive JSON
        .get("", find_all_users_controller)
        .get("/:id", find_by_id_controller)
        .put("/:id", update_user_controler)
        .delete("/:id", delete_by_id_controller)
}
```

### Static file serving

We can serve any folder as static files in any path of our server. This allows us to expose
basic files like a favicon.ico or complete Front-End applications statically compiled.

```rust
fn main() -> Result<(), ServerError> {
    Application::<Context>::builder()
        ...
        // we serve all of the files under the ./public folder in the base path of our 
        // application and all the files under ./static_views in the path /static
        .serve_static_files(
            StaticFileServer::new()
                .serve_folder("/", PathBuf::from("./public"))
                .serve_folder("/static", PathBuf::from("./static_views")),
        )
        .start()
        .await
}
```


### Templates
#### Template responses with [Tera](https://keats.github.io/tera/) as the template engine

With default support for Tera, we have a powerful template library inspired by Jinja2 and Django templates.
It automatically loads the templates available in the "templates" folder at the root of our project,
but this will be configurable via environment variables. When running with debug assertions,
the templates will be automatically reloaded when every request to a template endpoint is made, making development easier and faster.

```rust
// Application definition
fn main() -> Result<(), ServerError> {
    Application::<Context>::builder()
        ...
        .configure_tera(|mut tera| {
            tera.register_filter("url_encode", url_encode_filter);
            tera
        })
        .start()
        .await
}

// This is the handler for the / path. In this case we are going to return an HTML template
fn base_path_controller(context: Arc<Context>, _: Request) -> Response {
    match find_all_users(&mut context.get_db_connection()) {
        Ok(users) => Response::template("index.html", &UserListResponse { users }).unwrap(),
        Err(_) => Response::template("error.html", &json!({})).unwrap(),
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

```

### Security
#### Authorization API with support for OpenID Connect, simple JWT and custom configurations

Citrine provides an easy API to define which endpoints you want protected, freely allowed or completely denied.
It currently offers suppport for OpenID Connect, simple JWT validation, or a custom authentication method, 
with other options comming in the future.
With the current API, it provides the flexibility of choosing different authentication methods 
for any request or just assigning a default behaviour for all. You can add multiple request matchers
to a single SecurityRule by calling the matching_requests method multiple times.

##### Configuration as an OpenID Connect resource server

For this example we use Keycloak as an authorization server

```rust 
fn main() -> Result<(), ServerError> {
    Application::<Context>::builder()
        ...
        .security_configuration(
            SecurityConfiguration::new()
                // We protect writes in the /api subdomain but allow reads
                .add_rule(SecurityRule::new()
                        .matching_requests(
                            MethodMatcher::Multiple(vec![
                                Method::POST,
                                Method::PUT,
                                Method::DELETE,
                            ]),
                            "/api/*",
                        ).execute_action(SecurityAction::Authenticate(Authenticator::OIDC(OIDCConfiguration::new(
                                    HashSet::from([Uri::from_static("http://{keycloak_host}/realms/{your_realm}")]),
                                    Uri::from_static("http://{keycloak_host}/realms/{your_realm}/protocol/openid-connect/certs"),
                                    HashSet::from(["{your_audience}".to_string()])).await)
                )))
                // Any other request is allowed. This is the default behaviour if this line is
                // removed, but adding it makes it more explicit what you want to do with with
                // the requests that do not match the rules above
                .add_rule(MethodMatcher::All, "/*", SecurityAction::Allow),
        )
        .start()
        .await
}
```


##### Configuration with simple JWT validation
```rust 
fn main() -> Result<(), ServerError> {
    Application::<Context>::builder()
        ...
        .security_configuration(
            SecurityConfiguration::new()
                // We protect writes in the /api subdomain but allow reads
                .add_rule(
                    SecurityRule::new()
                        .matching_requests(
                            MethodMatcher::Multiple(vec![
                                Method::POST,
                                Method::PUT,
                                Method::DELETE,
                            ]),
                            "/api/*",
                        )
                        .execute_action(SecurityAction::Authenticate(Authenticator::JWT(
                            JWTConfiguration::new(
                                JWTSecret::base64_encoded(jwt_secret),
                                Algorithm::HS256,
                            ),
                        ))),
                ) 
                // Any other request is allowed. This is the default behaviour if this line is
                // removed, but adding it makes it more explicit what you want to do with with
                // the requests that do not match the rules above
                .add_rule(MethodMatcher::All, "/*", SecurityAction::Allow),
        )
        .start()
        .await
}
```

### Multiple request types

When creating a route, we can specify the content types we support. We can support multiple 
content types in the same endpoint, like an URL encoded form and JSON. If the Content-Type 
does not match, a 415 error will be automatically sent to the client. In the handler however,
reading the body is transparent to the Content-Type specified.

We can also specify whether we want to validate the body when reading it. For this feature
to work, the request body struct must derive Validate.

```rust

// Create user handler
fn create_user_controler(context: Arc<Context>, req: Request) -> Response {
    match req.get_body_validated::<CreateUser>() {
        Ok(create_user_request) => {
            match create(create_user_request.into(), &mut context.get_db_connection()) {
                Ok(_) => Response::new(StatusCode::NO_CONTENT),
                Err(e) => Response::default_error(&e),
            }
        }
        Err(e) => e.into(),
    }
}

// Update user handler
fn update_user_controler(context: Arc<Context>, req: Request) -> Response {
    match req.get_body_validated::<UpdateUser>() {
        Ok(update_user_request) => {
            match update(
                req.get_path_variables().get("id").unwrap(),
                update_user_request,
                &mut context.get_db_connection(),
            ) {
                Ok(_) => Response::new(StatusCode::NO_CONTENT),
                Err(e) => Response::default_error(&e),
            }
        }
        Err(e) => e.into(),
    }
}
```

### Request middlewares and response interceptor

For logging or other purposes, Citrine provides two tools, request middlewares and a response interceptor function.

Request middlewares will be executed just before a request reaches the handler, allowing you to log it or 
modify it as you please. You can filter which middleware each function uses via request matchers, just
like the security configuration. Each request will enter just one middleware, the first one that matches in definition order.
All requests must have passed the authorization filter and not be static file requests, because they will have already been served.

The response interceptor function will be executed after every request, giving read access to the request and response. 
```rust
fn main() -> Result<(), ServerError> {
    Application::<Context>::builder()
        ...
        .request_middleware(
            RequestMiddleware::new()
                .add_middleware(MethodMatcher::All, "/api/*", |request| {
                    info!("API Request: {} {}", request.method, request.uri,);
                    request
                })
                .add_middleware(MethodMatcher::All, "/*", |request| {
                    info!("Template request {} {}", request.method, request.uri);
                    request
                }),
        )
        .response_interceptor(|request, response| {
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
        .start()
        .await
}
```

### Configuration via environment variables

Some basic configuration options can be set via environment variables. These are:

* `CITRINE_PORT`: Sets the port the application will listen to. Default is `8080`.
* `CITRINE_APP_NAME`: The application name that will appear on startup. If none is set it will
use the name of the crate.
* `CITRINE_TEMPLATES_ENABLED`: Whether the framework will load the templates on startup. Default is `false`.
* `CITRINE_TEMPLATES_FOLDER`: The folder that contains the application templates. Default is `templates`.
* `CITRINE_BANNER_ENABLED`: Whether the framework will show a banner when starting the application. Default is `true`.

These configurations can also be set using the application builder. If both options are used at the same
time, the values set in the code will prevail.

### Startup Banner

Show a custom banner when the application starts by creating a `banner.txt` file in the root of your project.
This can be disabled via environment variables.

![image](https://github.com/user-attachments/assets/124b69c3-43da-4e78-ab09-a1c4ddf7cae2 "Default banner")
*Default banner*

![image](https://github.com/user-attachments/assets/ead37db0-fd76-4a2c-844b-0a5ea29a21be "Custom banner")
*Custom banner*


--- 

## To Do Before MVP

This list is subject to change. For a more accurate representation take a look at the issues with the label 'mvp'

- [ ] **Better configuration.** Offer environment variables for things like the templates folder,
default port, application name and version, and others.
- [ ] **Better authorization options.** This could be SAML support or integration with SSO services like Keycloak.
- [ ] **Opt-in features.** Things like the security API or the Tera template engine should be opt-in features instead
of being bundled with the framework by default.
- [ ] **Optimization.** Performance is key in a web framework, even more if we are in Rust. Lots of work
should be put into increasing speed and decreasing memory footprint in production mode.
- [ ] **Testing.** More in depth testing and the setting up of a testing CI pipeline on push/merge.
- [ ] **Documentation.** In depth documentation and usage examples of the framework. It should be started 
early even if there is little to document by now.

---

## Planned features

This could come either before or after the above features. This list is also subject to change, check the
issues page for a more precise context.

- [ ] GraphQL support
- [ ] DEV UI
- [ ] Use a new HTTP Server instead of Hyper
