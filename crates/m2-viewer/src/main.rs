fn main() {
    match m2_viewer::parse_args(std::env::args().skip(1)) {
        Ok(args) => {
            m2_viewer::build_app(&args).run();
        }
        Err(e) => {
            eprintln!(
                "m2-viewer: {e}\nusage: m2-viewer [client dir] [--index file] [--motion <virtual path>] \
                 [--screenshot <virtual path> <out.png>]"
            );
            std::process::exit(2);
        }
    }
}
