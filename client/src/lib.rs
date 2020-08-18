use seed::{prelude::*, *};

// Paths
const LOGIN: &str = "login";

// ------ ------
//     Init
// ------ ------

fn init(url: Url, orders: &mut impl Orders<Msg>) -> Model {
    orders
        .subscribe(Msg::UrlChanged)
        .send_msg(Msg::CheckAuth);

    let user = User::Loading;
    Model {
        base_url: url.to_base_url(),
        page: Page::init(url, &user),
        user,
    }
}

// ------ ------
//     Model
// ------ ------

struct Model {
    base_url: Url,
    page: Page,
    user: User,
}

enum User {
    Anonymous,
    Loading,
    Loaded(String),
}

// The idea is to follow the same pattern as the seed
// auth example.  Only, instead of getting a LoggedUser
// directly from the login response, it must be fetched
// from a call to the /auth/check endpoint.  The server
// doesn't care who seed thinks the user is; it will
// authenticate requests _only_ using the actix-identity::Identity
// cookie sent from the browser http-only cache.
enum Page {
    Login { username: String, password: String },
    Dashboard,
    NotFound,
}

impl Page {
    fn init(mut url: Url, user: &User) -> Self {
        match user {
            User::Anonymous => {
                Self::Login {
                    username: String::new(),
                    password: String::new(),
                }
            }
            User::Loading | User::Loaded(_) => {
                match url.next_path_part() {
                    None => Self::Dashboard,
                    Some(LOGIN) => Self::Login {
                        username: String::new(),
                        password: String::new(),
                    },
                    Some(_) => Self::NotFound,
                }
            }
        }
    }
}

// ------ ------
//     Urls
// ------ ------

struct_urls!();
impl<'a> Urls<'a> {
    pub fn home(self) -> Url {
        self.base_url()
    }
    pub fn login(self) -> Url {
        self.base_url().add_path_part(LOGIN)
    }
}

// ------ ------
//    Update
// ------ ------

enum Msg {
    // basic switching between a /login page and
    // a / home page
    UrlChanged(subs::UrlChanged),

    // /auth/login messages
    UpdateLoginUser(String),
    UpdateLoginPass(String),
    Login,
    LoginResponse(fetch::Result<String>),
    Logout,
    LogoutResponse(fetch::Result<()>),

    // /auth/check messages
    CheckAuth,
    AuthStatus(fetch::Result<String>),
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::UrlChanged(subs::UrlChanged(url)) => {
            model.page = Page::init(url, &model.user);
        }
        Msg::CheckAuth => {
            // check with the server whether or not we have
            // a secure session cookie.
            // Do this before the seed model knows that it's
            // logged in, such as on startup, or with any url
            // change before a successful login is submitted
            log!("checking auth");
            orders.perform_cmd(async {
                Msg::AuthStatus(
                    async {
                        Request::new("/auth/check")
                            .timeout(5_000)
                            .fetch()
                            .await?
                            .check_status()?
                            .text()
                            .await
                    }
                    .await,
                )
            });
        }
        Msg::AuthStatus(Ok(user)) => {
            log!("auth status ok:", user);
            if user.is_empty() {
                model.user = User::Anonymous;
                request_url(Urls::new(&model.base_url).login(), orders);
            } else {
                model.user = User::Loaded(user);
            }
        }
        Msg::AuthStatus(Err(e)) => {
            #[cfg(debug_assertions)]
            log!("Error checking auth:", e);
            model.user = User::Anonymous;
            request_url(Urls::new(&model.base_url).login(), orders);
        }
        Msg::UpdateLoginUser(user) => {
            if let Page::Login { username, ..} = &mut model.page {
                *username = user;
            }
        }
        Msg::UpdateLoginPass(pass) => {
            if let Page::Login { password, ..} = &mut model.page {
                *password = pass;
            }
        }
        Msg::Login => {
            let (username, password) = match &model.page {
                Page::Login { username, password, ..} => (username.clone(), password.clone()),
                _ => return
            };

            // Here is where the problem appears to be.  I want the
            // app to wait until it has a LoginResponse from the server
            // before doing _anything_.  Yet things happen immediately
            // after login is clicked -- the page refreshes, and auth
            // status is check again _before_ any response from the server
            // has arrived!  I don't understand why!  Any help explaining
            // this would be appreciated...
            orders.perform_cmd(async move {
                Msg::LoginResponse(
                    async {
                        Request::new("/auth/login")
                            .header(Header::authorization(format!(
                                "Basic {}",
                                base64::encode(format!("{}:{}", username, password))
                            )))
                            .timeout(5_000)
                            .fetch()
                            .await?
                            .check_status()?
                            .text()
                            .await
                    }
                    .await,
                )
            });
        }
        Msg::LoginResponse(Ok(user)) => {
            // If there is an Ok response from out login request, great!
            // We should have a session cookie in the browser and we
            // can then go to our home page and have the app check our
            // auth status successfully.  If not, then we need to try to
            // log in again.
            log!("auth status ok:", user);
            if user.is_empty() {
                model.user = User::Anonymous;
                request_url(Urls::new(&model.base_url).login(), orders);
            } else {
                model.user = User::Loaded(user);
                request_url(Urls::new(&model.base_url).home(), orders);
            }
        }
        Msg::LoginResponse(Err(e)) => {
            #[cfg(debug_assertions)]
            log!("Error checking auth:", e);
            model.user = User::Anonymous;
            request_url(Urls::new(&model.base_url).login(), orders);
        }
        Msg::Logout => {
            orders.perform_cmd(async move {
                Msg::LogoutResponse(
                    async {
                        Request::new("/auth/logout")
                            .fetch()
                            .await?
                            .check_status()?;
                        Ok(())
                    }
                    .await,
                )
            });
        }
        Msg::LogoutResponse(Ok(())) => {
            log!("User has been logged out.");
            model.user = User::Anonymous;
            request_url(Urls::new(&model.base_url).login(), orders);
        }
        Msg::LogoutResponse(Err(e)) => {
            error!("Log out failed:", e);
        }
    }
}

fn request_url(url: Url, orders: &mut impl Orders<Msg>) {
    orders.notify(subs::UrlRequested::new(url));
}

// ------ ------
//     View
// ------ ------

fn view(model: &Model) -> Node<Msg> {
    match &model.page {
        Page::Login {
            username, password, ..
        } => div![
            h1!["Login"],
            form![
                ev(Ev::Submit, move |event| {
                    event.prevent_default();
                    Msg::Login
                }),
                label!["Enter your email:"],
                input![
                    attrs! {
                        At::Value => username;
                        At::Placeholder => "me@example.com";
                    },
                    input_ev(Ev::Input, Msg::UpdateLoginUser),
                ],
                label!["Enter your password:"],
                input![
                    attrs! {
                        At::Value => password;
                        At::Type => "password";
                        At::Placeholder => "password";
                    },
                    input_ev(Ev::Input, Msg::UpdateLoginPass),
                ],
                button!["Log In"],
            ],
        ],
        Page::Dashboard => {
            if let User::Loaded(user) = &model.user {
                div![
                    h1![format!("{} 's Dashboard", user)],
                    button![
                        "Log Out",
                        ev(Ev::Click, |_| Msg::Logout),
                    ],
                ]
            } else {
                div!["unexpected model state"]
            }
        }
        Page::NotFound => div!["Page not found"],
    }
}

// ------ ------
//     Start
// ------ ------

#[wasm_bindgen(start)]
pub fn start() {
    App::start("app", init, update, view);
}
