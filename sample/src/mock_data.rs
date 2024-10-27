use crate::User;

pub fn get_database_creation_query() -> String {
    "CREATE TABLE Users (
        id TEXT PRIMARY KEY,
        username TEXT NOT NULL,
        mail TEXT NOT NULL,
        profile_picture_url TEXT NOT NULL
    );".to_string()
}

pub fn get_mock_users() -> Vec<User> {
    vec![
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
    ]
}

