use std::env::var;
use std::fs;
use std::str::FromStr;

// generated by `sqlx migrate build-script`
fn main() {
	let toml_file = fs::read_to_string("./config-defaults.toml").expect("Failed to read backend/config-defaults.toml");
	let table = toml::Table::from_str(&toml_file).unwrap();
	let mut sample_config = DEFAULT_SAMPLE.to_owned();
	for (key, value) in table.iter() {
		let formatted_value = if let Some(string) = value.as_str() {
			string.to_owned()
		} else {
			value.to_string()
		};
		println!("cargo:rustc-env={}={}", key.to_uppercase(), formatted_value); // toml returns strings with quotation marks, so we trim them
		sample_config = sample_config.replace(&format!("_{}", key.to_uppercase()), &value.to_string());
	}
	fs::write(var("OUT_DIR").unwrap() + "/config.toml.sample", sample_config).unwrap();


	#[cfg(feature = "integrated-frontend")]
	static_files::resource_dir("../frontend/dist").build().unwrap();

	println!("cargo:rerun-if-changed={}/config.toml.sample", var("OUT_DIR").unwrap());
	println!("cargo:rerun-if-changed=migrations"); // trigger recompilation when a new migration is added
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=config-defaults.toml");
}


const DEFAULT_SAMPLE: &'static str = r#"
# The URL where the server should bind to
# Optional; default is '127.0.0.1'.
# listen_url = _LISTEN_URL_DEFAULT

# The port where the server should bind to
# Optional; default is '7999'.
# port = _PORT_DEFAULT

# This is the url that will be prefixed to the shortened ID.
# The shortened link ajnIUh1H in the server response will look like `$public_url/ajnIUh1H`.
# If the public url is `short.example.com` the link the server will respond with will be `http://shorty.example.com/ajnIUh1H`.
# It is different from the listen_url if shorty is run behind a reverse proxy.
public_url = 'http://localhost:7999'

# Where the server should look for the database
database_location = 'database.db'

# The maximum length a link may have.
# Optional; default is 500 chars length.
# max_link_length = _MAX_LINK_LENGTH_DEFAULT

# The maximum json payload size for custom links in bytes.
# Optional; default is 2 MB
# max_json_size = _MAX_JSON_SIZE_DEFAULT

# Maximum length for custom IDs.
# Optional; default is 2500
# max_custom_id_length = _MAX_CUSTOM_ID_LENGTH_DEFAULT


# The link defaults that get used if they aren't specified.

# How often a link is able to be used before it expires.
# Optional, default is 0.
# default_max_uses = _MAX_USES_DEFAULT # Zero means unlimited uses.

# How long a link (by default) is valid for, in milliseconds.
# Optional, default is 7 days.
# default_valid_for = _VALID_FOR_DURATION_DEFAULT # 24 hours

# Location of custom frontend.
# If set, files in the folder will be served instead of the embedded frontend.
# frontend_location = '/var/www/shorty_frontend'
"#;