use seed::{prelude::*, *};

// ------ ------
//     Init
// ------ ------

fn init(url: Url, orders: &mut impl Orders<Msg>) -> Model {
    orders.subscribe(Msg::UrlChanged);
    let model = Model {
        base_url: url.to_base_url(),
        page: Page::for_url(url),
        user: None,
    };

    if model.user.is_none() {
        orders.send_msg(Msg::CheckAuth);
    }

    model
}

// ------ ------
//     Model
// ------ ------

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
    fn for_url(mut url: Url) -> Self {
        match url.next_path_part() {
            None => Self::Dashboard,
            Some("login") => Self::Login {
                username: Default::default(),
                password: Default::default(),
            },
            Some(_) => Self::NotFound,
        }
    }
}

struct Model {
    base_url: Url,
    page: Page,
    user: Option<String>,
}

// ------ ------
//     Urls
// ------ ------

struct_urls!();
impl<'a> Urls<'a> {
    pub fn home(self) -> Url {
        self.base_url()
    }
}

// ------ ------
//    Update
// ------ ------

enum Msg {
    // basic switching between a /login page and
    // a / home page
    GoToUrl(Url),
    UrlChanged(subs::UrlChanged),

    // /auth/login messages
    UpdateLoginUser(String),
    UpdateLoginPass(String),
    Login,
    LoginResponse(fetch::Result<String>),

    // /auth/check messages
    CheckAuth,
    AuthStatus(fetch::Result<String>),
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::GoToUrl(url) => {
            orders.notify(subs::UrlRequested::new(url));
        }
        Msg::UrlChanged(subs::UrlChanged(url)) => {
            // the idea here is to prevent a non-logged-in
            // user from going anywhere but the login screen.
            // This _may_ be the root of my problem (???);
            // my app is triggering browser refreshes that
            // are mysterious to me...
            if model.user.is_some() {
                model.page = Page::for_url(url);
            } else if url.path().is_empty() {
                orders.send_msg(Msg::CheckAuth);
            } else if url.path()[0] != "login" {
                orders.send_msg(Msg::GoToUrl(model.base_url.clone().add_path_part("login")));
            } else {
                model.page = Page::for_url(url);
            }
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
                model.user = None;
                orders.send_msg(Msg::GoToUrl(model.base_url.clone().add_path_part("login")));
            } else {
                model.user = Some(user);
                orders.send_msg(Msg::GoToUrl(model.base_url.clone()));
            }
        }
        Msg::AuthStatus(Err(e)) => {
            #[cfg(debug_assertions)]
            log!("Error checking auth:", e);
            model.user = None;
            orders.send_msg(Msg::GoToUrl(model.base_url.clone().add_path_part("login")));
        }
        Msg::UpdateLoginUser(user) => {
            if let Model {
                page: Page::Login { username, .. },
                ..
            } = model
            {
                *username = user;
            }
        }
        Msg::UpdateLoginPass(pass) => {
            if let Model {
                page: Page::Login { password, .. },
                ..
            } = model
            {
                *password = pass;
            }
        }
        Msg::Login => {
            if let Model {
                page: Page::Login {
                    username, password, ..
                },
                ..
            } = model
            {
                let username = username.clone();
                let password = password.clone();

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
        }
        Msg::LoginResponse(Ok(user)) => {
            // If there is an Ok response from out login request, great!
            // We should have a session cookie in the browser and we
            // can then go to our home page and have the app check our
            // auth status successfully.  If not, then we need to try to
            // log in again.
            log!("auth status ok:", user);
            if user.is_empty() {
                model.user = None;
                orders.send_msg(Msg::GoToUrl(model.base_url.clone().add_path_part("login")));
            } else {
                model.user = Some(user);
                orders.send_msg(Msg::GoToUrl(model.base_url.clone()));
            }
        }
        Msg::LoginResponse(Err(e)) => {
            #[cfg(debug_assertions)]
            log!("Error checking auth:", e);
            model.user = None;
            orders.send_msg(Msg::GoToUrl(model.base_url.clone().add_path_part("login")));
        }
    }
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
                button!["Log In", ev(Ev::Click, |_| Msg::Login),],
            ],
        ],
        Page::Dashboard => {
            if let Model {
                user: Some(user), ..
            } = model
            {
                div![
                    h1![format!("{} 's Dashboard", user)],
                    button![
                        "Log Out",
                        // ev(Ev::Click, |_| Msg::Logout),
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
