
# TASKS

### Current

### To Do 

#### MVP

- [ ] make deserialization dependant on request Content-Type. Use Accept-Type in request
- [ ] Improve sample error handling and find all request

#### Design

- [ ] Think of the best way of having request error handling. If either leave it completely to the 
user or let them override a function pointer to have centralized and customizable handling provided by the framework
- [ ] Request validation
- [ ] Async request handlers

#### Future

- [ ] HTTP2 support
- [ ] GRPC support


### Done

- [x] Add database connection to sample
- [x] Response interceptor
- [x] Separate body from json method in response
- [x] Content-Type in json response
- [x] Don't throw error from router. Return internal errors as responses
- [x] Should internal thrown errors like a 404 be logged? they are now logged if the level is debug
- [x] Variable url reading
- [x] Check if the code below is a valid error when adding route in route.rs (It was not)
```
        let routes: Vec<String> = route.split("/").map(|s| s.to_string()).collect();
        if routes.is_empty() {
            return Err(ServerError::from("Can't define an empty route"));
        }
```
