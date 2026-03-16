CREATE TABLE lios (
    id CHAR(36) PRIMARY KEY,
    provider VARCHAR(255) NOT NULL,
    provider_id VARCHAR(255) NOT NULL,
    station VARCHAR(255) NOT NULL,
    line VARCHAR(255) NOT NULL,
    direction VARCHAR(255) NOT NULL
);
