use error_chain::error_chain;
//use quick_error::quick_error;

error_chain!{

	errors {
		ResticRepoNotFound {
			description("Restic repository not found at given path")
			display("Restic repository not found at given path")
		}
		ResticRepoInvalidPassword {
			description("Restic repository is not decrypted with this password")
			display("Restic repository is not decrypted with this password")
		}
	}
}
//
//quick_error! {
//
//}
