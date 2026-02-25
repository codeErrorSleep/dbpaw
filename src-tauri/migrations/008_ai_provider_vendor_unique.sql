UPDATE ai_providers
SET provider_type = CASE
    WHEN lower(provider_type) = 'openai' THEN 'openai'
    WHEN lower(provider_type) = 'kimi' THEN 'kimi'
    WHEN lower(provider_type) = 'glm' THEN 'glm'
    WHEN lower(provider_type) = 'openai_compat' THEN CASE
        WHEN lower(name) LIKE '%kimi%' OR lower(base_url) LIKE '%moonshot%' THEN 'kimi'
        WHEN lower(name) LIKE '%glm%' OR lower(base_url) LIKE '%bigmodel%' OR lower(base_url) LIKE '%zhipu%' THEN 'glm'
        ELSE 'openai'
    END
    ELSE CASE
        WHEN lower(name) LIKE '%kimi%' OR lower(base_url) LIKE '%moonshot%' THEN 'kimi'
        WHEN lower(name) LIKE '%glm%' OR lower(base_url) LIKE '%bigmodel%' OR lower(base_url) LIKE '%zhipu%' THEN 'glm'
        ELSE 'openai'
    END
END;

DELETE FROM ai_providers
WHERE id IN (
    SELECT id
    FROM (
        SELECT
            id,
            ROW_NUMBER() OVER (
                PARTITION BY provider_type
                ORDER BY datetime(updated_at) DESC, id DESC
            ) AS rn
        FROM ai_providers
    ) ranked
    WHERE rn > 1
);

UPDATE ai_providers
SET is_default = CASE
    WHEN id = (
        SELECT id
        FROM ai_providers
        ORDER BY is_default DESC, datetime(updated_at) DESC, id DESC
        LIMIT 1
    ) THEN 1
    ELSE 0
END;

CREATE UNIQUE INDEX IF NOT EXISTS idx_ai_providers_provider_type_unique
ON ai_providers(provider_type);

CREATE TRIGGER IF NOT EXISTS trg_ai_providers_provider_type_insert
BEFORE INSERT ON ai_providers
FOR EACH ROW
WHEN NEW.provider_type NOT IN ('openai', 'kimi', 'glm')
BEGIN
    SELECT RAISE(ABORT, 'provider_type must be one of: openai, kimi, glm');
END;

CREATE TRIGGER IF NOT EXISTS trg_ai_providers_provider_type_update
BEFORE UPDATE OF provider_type ON ai_providers
FOR EACH ROW
WHEN NEW.provider_type NOT IN ('openai', 'kimi', 'glm')
BEGIN
    SELECT RAISE(ABORT, 'provider_type must be one of: openai, kimi, glm');
END;
