//!
//! # Iron X
//!
//! Simple framework for creating applications
//!

pub use app_container::AppContainer;
pub use application::Application;
pub use command::Command;
pub use error_compatible::ErrorCompatible;
pub use resource::Resource;
pub use runtime::{BorrowedRuntime, Runtime};
pub use serde_compatible::SerdeCompatible;
pub use stable::Stable;

mod stable {
    use std::fmt::Debug;

    /// # Stable Trait
    ///
    /// Tag trait for types that are easy to recognize, safe to
    /// to use in application context, and can be shared across
    /// threads.
    ///
    pub trait Stable: Send + Sync + Debug + Clone + 'static {}

    impl<T: Send + Sync + Debug + Clone + 'static> Stable for T {}
}
mod serde_compatible {
    use ::serde::{Deserialize, Serialize};

    /// # Serialize/Deserialize Compatible
    ///
    /// Tag trait for types that are required to be convertible
    /// both **from** and **into** different data representations
    /// such as JSON or YAML.
    ///
    pub trait SerdeCompatible: for<'de> Deserialize<'de> + Serialize {}

    impl<T: for<'de> Deserialize<'de> + Serialize> SerdeCompatible for T {}
}
mod error_compatible {
    use std::error::Error;

    /// # Error Compatible
    ///
    /// Tag trait to ensure all errors have at minimum implemented
    /// [Error] from the standard library.  More requirements may
    /// come later as error handling becomes more of a concern.
    ///
    pub trait ErrorCompatible: Error {}

    impl<T: Error> ErrorCompatible for T {}
}
mod resource {
    use super::Stable;

    /// # Resource Trait
    ///
    /// Represents access resource that can be used by
    /// your application or library.
    ///
    pub trait Resource<T>: Stable {
        fn resource(&self) -> &T;
    }

    impl<T: Stable> Resource<T> for T {
        fn resource(&self) -> &T {
            self
        }
    }
}
mod application {
    use crate::{ErrorCompatible, SerdeCompatible, Stable};

    /// # Application Trait
    ///
    /// Serves as a base trait for all applications.  This trait
    /// ensures the following cornerstones of an application:
    ///
    /// - can be initialized by a configuration
    /// - the configuration can be serialized and deserialized
    /// - an enforced abstract contract to it's environment
    /// - an enforced abstract conctract to running app context
    /// - access to an async compatibile, context bounded, runtime
    ///
    pub trait Application: Sized + Stable {
        /// Configuration needed by the application to run
        type Config: Stable + SerdeCompatible;

        /// Error type that is compatible with the application
        type Error: ErrorCompatible;

        /// State that the application needs to run
        type Env: Stable;

        /// Context state used by the [Runtime]
        type Ctx: Stable;

        /// # Initialization
        ///
        /// Function to initialize the application
        /// with a given configuration specified by
        /// the [Application::Config] type.
        ///
        fn init(config: Self::Config) -> impl Future<Output = Result<Self, Self::Error>>;

        /// # Fetch Environment
        ///
        /// Provides access to the application's
        /// environment for different actions.
        ///
        fn env(&self) -> &Self::Env;
    }
}
mod command {
    use crate::{Application, ErrorCompatible, SerdeCompatible, Stable};

    /// # Command Trait
    ///
    /// The command trait is used to abstract functionality that
    /// is intended to be run by the application [crate::Runtime].
    /// By capturing the context and environment of a command's
    /// supporting application, it allows for the command to run
    /// for multiple applications without needing to be defined
    /// again.
    ///
    pub trait Command<App: Application>: Stable + SerdeCompatible {
        /// Success case the command returns
        type Success: SerdeCompatible;

        /// Failure type that this command can return
        type Failure: ErrorCompatible;

        /// Asynchronous function that must be implemented for
        /// this command to run.  It receives the context and
        /// environment of the application and must return a
        /// result with the success or failure type of it's trait
        /// signature.
        ///
        fn call(
            &self,
            ctx: &App::Ctx,
            env: &App::Env,
        ) -> impl Future<Output = Result<Self::Success, Self::Failure>>;
    }
}
mod runtime {
    use crate::{Application, Command};

    /// # Runtime Trait
    ///
    /// Provides an interface to interact with an application.
    ///
    pub trait Runtime<App: Application> {
        fn run_command<T>(&self, cmd: &T) -> impl Future<Output = Result<T::Success, T::Failure>>
        where
            T: Command<App>;
    }

    /// # Borrowed Application Runtime
    ///
    /// A borrowed [Runtime] with an [Application::Ctx]
    ///
    #[derive(Debug, Clone)]
    pub struct BorrowedRuntime<'a, App>
    where
        App: Application,
    {
        application: &'a App,
        context: &'a App::Ctx,
    }

    impl<'a, App: Application> Runtime<App> for BorrowedRuntime<'a, App> {
        async fn run_command<T>(&self, cmd: &T) -> Result<T::Success, T::Failure>
        where
            T: Command<App>,
        {
            cmd.call(self.context, self.application.env()).await
        }
    }

    impl<'a, App> BorrowedRuntime<'a, App>
    where
        App: Application,
    {
        #[doc(hidden)]
        pub(crate) fn new(application: &'a App, context: &'a App::Ctx) -> Self {
            Self {
                application,
                context,
            }
        }
    }
}
mod app_container {
    use crate::{Application, BorrowedRuntime, Runtime};

    /// # Application Container
    ///
    /// A layer of abstraction that serves as a default [Runtime]
    /// with it's provied [Self::default_context] as the context
    /// for is it's lifetime.  This allows the application to have
    /// a default context which can be used to run commands where
    /// such a context is required, without needing to provide a
    /// default context every time a command is run.
    ///
    #[derive(Debug, Clone)]
    pub struct AppContainer<App: Application> {
        app: App,
        default_context: App::Ctx,
    }

    impl<App: Application> AppContainer<App> {
        pub fn with_default_context(ctx: App::Ctx) -> AppContainerBuilder<App> {
            AppContainerBuilder {
                default_context: ctx,
            }
        }

        pub fn with_context<'a>(&'a self, ctx: &'a App::Ctx) -> BorrowedRuntime<'a, App> {
            BorrowedRuntime::new(&self.app, ctx)
        }
    }

    impl<App: Application> Runtime<App> for AppContainer<App> {
        async fn run_command<T>(&self, cmd: &T) -> Result<T::Success, T::Failure>
        where
            T: crate::Command<App>,
        {
            cmd.call(&self.default_context, self.app.env()).await
        }
    }

    #[derive(Debug)]
    pub struct AppContainerBuilder<App: Application> {
        default_context: App::Ctx,
    }

    impl<App: Application> AppContainerBuilder<App> {
        pub async fn init(self, config: App::Config) -> Result<AppContainer<App>, App::Error> {
            let app = App::init(config).await?;
            Ok(AppContainer {
                app,
                default_context: self.default_context,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    struct GreetingsFrom {
        pub location: String,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    struct Host(String);

    #[derive(Debug, Serialize, Deserialize, Clone)]
    struct Vistor(String);

    #[derive(Debug, Serialize, Deserialize, Clone)]
    struct Greetings(Host);

    #[derive(Debug, ::thiserror::Error)]
    enum GeetingsErr {}

    impl Application for Greetings {
        type Config = String;
        type Error = GeetingsErr;
        type Env = Host;
        type Ctx = Vistor;

        async fn init(config: Self::Config) -> Result<Self, Self::Error> {
            Ok(Self(Host(config)))
        }

        fn env(&self) -> &Self::Env {
            &self.0
        }
    }

    impl<App> Command<App> for GreetingsFrom
    where
        App: Application,
        App::Ctx: Resource<Vistor>,
        App::Env: Resource<Host>,
    {
        type Success = String;
        type Failure = std::fmt::Error;

        async fn call(
            &self,
            ctx: &App::Ctx,
            env: &App::Env,
        ) -> Result<Self::Success, Self::Failure> {
            use std::fmt::Write;

            let mut message = String::new();
            write!(
                &mut message,
                "Hello {name}, welcome to {location} on behalf of {host}!",
                name = ctx.resource().0.as_str(),
                location = &self.location,
                host = env.resource().0.as_str(),
            )?;
            Ok(message)
        }
    }

    #[tokio::test]
    async fn it_works() {
        let app = AppContainer::<Greetings>::with_default_context(Vistor("Alice".to_string()))
            .init("Iron X".to_string())
            .await
            .expect("Failed to initialize application");
        let messsage = app
            .run_command(&GreetingsFrom {
                location: "Rustland".to_string(),
            })
            .await
            .expect("Failed to run command");
        assert_eq!(
            messsage,
            "Hello Alice, welcome to Rustland on behalf of Iron X!"
        );
    }
}
