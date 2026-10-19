-- The first-launch setup flow arrived in v0.6.0. An install that already has a
-- device identity was set up before then, so mark it done; only a fresh
-- database, where the identity is created after migrations run, sees the flow.
INSERT OR IGNORE INTO settings (key, value)
SELECT 'onboarding_complete', 'true' WHERE EXISTS (SELECT 1 FROM local_device);
