ALTER TABLE connections ADD COLUMN ssh_enabled INTEGER DEFAULT 0;
ALTER TABLE connections ADD COLUMN ssh_host TEXT;
ALTER TABLE connections ADD COLUMN ssh_port INTEGER;
ALTER TABLE connections ADD COLUMN ssh_username TEXT;
ALTER TABLE connections ADD COLUMN ssh_password TEXT;
ALTER TABLE connections ADD COLUMN ssh_key_path TEXT;
