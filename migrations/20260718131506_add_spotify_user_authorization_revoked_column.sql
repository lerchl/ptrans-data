ALTER TABLE spotify_users ADD COLUMN authorization_revoked BOOLEAN NOT NULL DEFAULT FALSE AFTER scopes;
