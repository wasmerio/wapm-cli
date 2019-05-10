SELECT wu.name, public_key_tag 
FROM wapm_public_keys 
JOIN wapm_users wu ON user_key = wu.id
WHERE public_key_tag = (?1) 
   OR public_key_value = (?2)
