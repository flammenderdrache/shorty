use std::fmt::{Display, Formatter};

use chrono::Local;
use serde::Deserialize;
use sqlx::{Pool, Sqlite};
use tracing::debug;

use crate::{CONFIG, ensure_http_prefix};
use crate::error::ShortyError;
use crate::util::{get_random_id, replace_illegal_url_chars, time_now};

/// This struct holds configuration options for a custom link.
/// Optional fields are: `custom_id`, `max_uses`, and `valid_for`.
/// `valid_for` and `max_uses` default to 0, which means essentially infinite.
#[derive(Debug, Clone, Deserialize)]
pub struct LinkConfig {
	/// The link that should be shortened.
	pub link: String,
	/// Custom ID for the link (like when you want a word instead of random jumble of chars).
	#[serde(alias = "id")]
	custom_id: Option<String>,
	/// How often the link may be used.
	#[serde(default = "default_max_uses")]
	max_uses: i64,
	/// How long the link is valid for in milliseconds.
	#[serde(default = "default_valid_for")]
	valid_for: i64,
}

/// This function exists only because serde's default can't take values or a value from a struct.
fn default_max_uses() -> i64 {
	CONFIG.default_max_uses
}

/// This function exists only because serde's default can't take values or a value from a struct.
fn default_valid_for() -> i64 {
	CONFIG.default_valid_for
}

/// Struct representing a (shortened) Link.
/// All timestamps are in milliseconds.
#[derive(Debug, Clone)]
pub struct Link {
	pub id: String,
	pub redirect_to: String,
	max_uses: i64,
	invocations: i64,
	created_at: i64,
	valid_for: i64,
}

impl Display for Link {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.redirect_to)
	}
}

impl Link {
	/// Creates a new link with a default configuration.
	/// Just creates a default config and calls [`Link::new_with_config`] with it.
	///
	/// # Errors
	///
	/// Errors if the underlying [`Link::new_with_config`] errors.
	pub async fn new(
		link: String,
		pool: &Pool<Sqlite>,
	) -> Result<Self, ShortyError> {
		let link_config = LinkConfig {
			link,
			custom_id: None,
			max_uses: CONFIG.default_max_uses,
			valid_for: CONFIG.default_valid_for,
		};


		Link::new_with_config(link_config, pool).await
	}

	/// Creates a new link according to the config provided.
	///
	/// # Errors
	///
	/// Returns an error if the link with the requested ID already exists.
	/// Also returns an error if there was a problem executing the SQL queries.
	pub async fn new_with_config(
		link_config: LinkConfig,
		pool: &Pool<Sqlite>,
	) -> Result<Self, ShortyError> {
		let id = if let Some(id) = link_config.custom_id {
			if id.len() > CONFIG.max_custom_id_length {
				return Err(ShortyError::CustomIDExceedsMaxLength);
			}

			replace_illegal_url_chars(&id)
		} else {
			get_random_id(pool).await?
		};
		let redirect_to = link_config.link;
		let max_uses = link_config.max_uses;
		let invocations = 0;
		let created_at = time_now();
		let valid_for = link_config.valid_for;

		if redirect_to.is_empty() {
			return Err(ShortyError::LinkEmpty);
		}

		if redirect_to.len() > CONFIG.max_link_length {
			return Err(ShortyError::LinkExceedsMaxLength);
		}

		let redirect_to = ensure_http_prefix(redirect_to);

		// If a link with the same ID exists already, return a conflict error.
		if let Some(link) = Link::from_id_no_invocation(id.as_str(), pool).await? {
			if !link.is_expired() {
				return Err(ShortyError::LinkConflict);
			}
		}

		let shortened = Self {
			id,
			redirect_to,
			max_uses,
			invocations,
			created_at,
			valid_for,
		};

		if shortened.is_expired() {
			return Err(ShortyError::ExpiredLinkProvided);
		}

		// We checked if the link exists already and is valid.
		// If it exists it has to be stale and can be replaced.
		sqlx::query!(
			r#"
				INSERT OR REPLACE INTO links
				VALUES ($1, $2, $3, $4, $5, $6)
			"#,
			shortened.id,
			shortened.redirect_to,
			max_uses,
			invocations,
			created_at,
			valid_for
		)
			.execute(pool)
			.await?;


		Ok(shortened)
	}

	/// A link with a valid_for of 0 is considered non-expiring based on time.
	/// A link with max_uses of 0 is considered infinitely usable, as long as it hasn't
	/// expired time-wise.
	#[must_use]
	pub fn is_expired(&self) -> bool {
		let time_expired = self.valid_for < 0 || (self.valid_for > 0
			&& (Local::now().timestamp_millis() - self.created_at) > self.valid_for);

		let uses_invalid = self.max_uses < 0
			|| (self.max_uses > 0 && self.invocations >= self.max_uses);

		debug!("time_expired: {time_expired}");
		debug!("uses_invalid: {uses_invalid}");
		time_expired || uses_invalid
	}

	/// Retrieves a link from the database, if it exists.
	/// Calling this function also increments the invocations if the link exists in the database.
	async fn from_id(id: &str, pool: &Pool<Sqlite>) -> Result<Option<Self>, ShortyError> {
		let link = sqlx::query_as!(
			Self,
			r#"
			SELECT * FROM links
			WHERE id = $1;
			UPDATE links
			SET invocations = invocations + 1
			WHERE id = $2;
			"#,
			id,
			id
		)
			.fetch_optional(pool)
			.await?;


		Ok(link)
	}

	/// Retrieves a link from the database, if it exists.
	/// This function **does not** increment the invocation counter of a link.
	async fn from_id_no_invocation(id: &str, pool: &Pool<Sqlite>) -> Result<Option<Self>, ShortyError> {
		let link = sqlx::query_as!(
			Self,
			r#"
			SELECT * FROM links
			WHERE id = $1;
			"#,
			id,
		)
			.fetch_optional(pool)
			.await?;


		Ok(link)
	}

	/// Checks if the link exists in the database.
	///
	/// # Errors
	///
	/// Errors if there is some problem communicating with the database.
	pub async fn link_exists(id: &str, pool: &Pool<Sqlite>) -> Result<bool, ShortyError> {
		let link_row = sqlx::query!(r#"
			SELECT id FROM links WHERE id = ?;
		"#,
		id
		)
			.fetch_optional(pool)
			.await?;


		Ok(link_row.is_some())
	}

	/// Formats self, according to the options set in the config file.
	#[must_use]
	pub fn formatted(&self) -> String {
		format!("{}/{}", CONFIG.public_url, self.id)
	}
}

pub struct LinkStore {
	db: Pool<Sqlite>,
}

impl LinkStore {
	#[must_use]
	pub fn new(db: Pool<Sqlite>) -> Self {
		Self { db }
	}

	/// Retrieves a link with the provided ID, if it exists.
	pub async fn get(&self, id: &str) -> Option<Link> {
		let link = Link::from_id(id, &self.db).await;

		if let Ok(Some(link)) = link {
			if !link.is_expired() {
				return Some(link);
			}

			debug!("{} got requested but is expired.", link.id);
		}


		None
	}

	/// Creates a shortened link with default settings.
	///
	/// # Errors
	///
	/// Returns an error if the underlying [`Link::new`] call fails.
	pub async fn create_link(&self, link: String) -> Result<Link, ShortyError> {
		Link::new(link, &self.db).await
	}

	/// Creates a shortened link with custom settings.
	///
	/// # Errors
	///
	/// Returns an error if the underlying [`Link::new_with_config`] call fails.
	pub async fn create_link_with_config(
		&self,
		link_config: LinkConfig,
	) -> Result<Link, ShortyError> {
		Link::new_with_config(link_config, &self.db).await
	}

	/// This function deletes stale links from the database.
	///
	/// # Errors
	///
	/// Errors if theres a problem executing the SQL queries.
	pub async fn clean(&self) -> Result<(), ShortyError> {
		debug!("Clearing stale links");

		let res = sqlx::query!("SELECT COUNT(*) AS num_before FROM links").fetch_one(&self.db).await?;
		let num_before = res.num_before;

		let now = time_now();
		sqlx::query!(
			r#"
			DELETE FROM links
			WHERE max_uses != 0 AND invocations > max_uses
			OR created_at + valid_for < $1
			"#,
			now
		)
			.execute(&self.db)
			.await?;

		let res = sqlx::query!("SELECT COUNT(*) AS num_after FROM links").fetch_one(&self.db).await?;
		let num_after = res.num_after;

		let delta = num_before - num_after;
		debug!("Size before cleaning: {num_before}. After cleaning: {num_after}. Removed elements: {delta}");


		Ok(())
	}
}
