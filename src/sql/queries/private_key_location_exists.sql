SELECT private_key_location, public_key_value
FROM personal_keys
WHERE private_key_location = (?1)
