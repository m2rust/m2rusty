//! Headless boot of the C++ Python host from Rust, the Rust counterpart of
//! the host's TA.6 smoke test. One test: the interpreter is per process.

use m2_script::{Error, PythonHost};

#[test]
fn boots_and_runs_host_modules() {
    let host = PythonHost::start("m2-script.test").expect("interpreter starts");

    assert!(
        matches!(PythonHost::start("again"), Err(Error::Host(_))),
        "a second interpreter is refused"
    );

    host.run_line("assert TRUE == 1 and FALSE == 0").unwrap();

    host.run_line(
        "import dbg\n\
         dbg.Tracen('m2-script: hello from python')\n\
         dbg.TraceError('m2-script: error channel')\n",
    )
    .unwrap();

    host.run_line(
        "import time\n\
         t = time.time()\n\
         assert t > 1.6e9, t\n\
         assert len(time.strftime('%Y')) == 4\n",
    )
    .unwrap();

    host.run_line(
        "import math\n\
         assert math.sqrt(16.0) == 4.0\n\
         assert abs(math.sin(math.radians(90.0)) - 1.0) < 1e-9\n\
         assert math.floor(2.7) == 2.0\n",
    )
    .unwrap();

    // A raised exception comes back as an error, and the host keeps working.
    assert!(host.run_line("raise ValueError('expected')").is_err());
    host.run_line("x = 6 * 7\nassert x == 42").unwrap();

    assert_eq!(host.run_line("a\0b"), Err(Error::InteriorNul));
}
