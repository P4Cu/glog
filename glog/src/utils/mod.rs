pub trait WarnOnErr<E> {
    /// Simple trait which allows to handle errors by emiting a warn!() and consuming the error in the process.
    /// Works only for Result<(), E>.
    fn warn_on_err(self, message: &str);
}
impl<E> WarnOnErr<E> for Result<(), E>
where
    E: std::fmt::Display,
{
    fn warn_on_err(self, message: &str) {
        if let Err(ref error) = self {
            log::warn!("{} {}", message, error);
        }
    }
}
