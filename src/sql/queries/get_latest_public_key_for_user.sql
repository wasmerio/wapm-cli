SELECT public_key_id, public_key_value, date_added, key_type_identifier
FROM wapm_public_keys
JOIN wapm_users wu ON user_key = wu.id
WHERE wu.name = (?1)
ORDER BY date_added DESC
LIMIT 1
