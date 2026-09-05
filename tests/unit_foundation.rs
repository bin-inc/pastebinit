use pastebinit::{AppError, VERSION};

#[test]
fn version_and_operational_error_contract_are_stable() {
    assert_eq!(VERSION, "1.9.0-rc.1");
    assert_eq!(AppError::input("cannot read input").exit_code(), 1);
    assert_eq!(AppError::usage("invalid option").exit_code(), 2);
}
