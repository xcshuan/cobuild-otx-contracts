pub fn main() -> Result<(), crate::error::Error> {
    crate::runner::run(&crate::verify::local::LocalVerifier)
}
