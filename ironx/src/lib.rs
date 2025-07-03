//!
//! # Iron X
//!
//! Simple framework for creating applications
//!

pub use application::Application;
pub use command::Command;
pub use error_compatible::ErrorCompatible;
pub use resource::Resource;
pub use runtime::Runtime;
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
}
mod application {
    use crate::{ErrorCompatible, Runtime, SerdeCompatible, Stable};

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

        /// # Build Runtime
        ///
        /// With the provided context a [Runtime] is
        /// returned that provides access to interact
        /// with the application.
        ///
        fn build_runtime<'a>(&'a self, context: &'a Self::Ctx) -> Runtime<'a, Self> {
            Runtime::new(self, context)
        }
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

    /// # Application Runtime
    ///
    /// Provides an interface to interact with an application.
    ///
    #[derive(Debug, Clone)]
    pub struct Runtime<'a, App>
    where
        App: Application,
    {
        application: &'a App,
        context: &'a App::Ctx,
    }

    impl<'a, App> Runtime<'a, App>
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

        /// # Run Command
        ///
        /// Provides an interface to run a [Command] that is
        /// compatible with the application that provided it.
        ///
        pub async fn run_command<T>(&self, cmd: &T) -> Result<T::Success, T::Failure>
        where
            T: Command<App>,
        {
            cmd.call(self.context, self.application.env()).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Clone)]
    struct GreetingFrom {
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

    impl Resource<Vistor> for Vistor {
        fn resource(&self) -> &Vistor {
            self
        }
    }

    impl Resource<Host> for Host {
        fn resource(&self) -> &Host {
            self
        }
    }

    impl<App> Command<App> for GreetingFrom
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
        let greeter = Greetings::init("TestApp".into()).await.unwrap();
        let context = Vistor("test-user".into());

        let message = greeter
            .build_runtime(&context)
            .run_command(&GreetingFrom {
                location: "Timbuktu".into(),
            })
            .await
            .unwrap();

        assert_eq!(
            message,
            "Hello test-user, welcome to Timbuktu on behalf of TestApp!"
        );
    }
}
