use error_chain::error_chain;
//use quick_error::quick_error;

error_chain! {

    errors {
        ResticRepoNotFound {
            description("Restic repository not found at given path")
            display("Restic repository not found at given path")
        }
        ResticRepoInvalidPassword {
            description("Restic repository is not decrypted with this password")
            display("Restic repository is not decrypted with this password")
        }
        InvalidId {
            description("The input id does not contain all hex characters")
            display("The input id does not contain all hex characters")
        }
        NoOutputFromRestic {
            description("Restic output does not contain any output?")
            display("Restic output does not contain any output?")
        }
    }
}
//
//quick_error! {
//
//}
