SELECT wu.name, public_key_value, date_added, key_type_identifier, public_key_id 
FROM wapm_public_keys
JOIN wapm_users wu ON user_key = wu.id
ORDER BY date_added;
