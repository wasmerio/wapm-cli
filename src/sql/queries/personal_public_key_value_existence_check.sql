SELECT public_key_id
FROM personal_keys
WHERE public_key_id = (?1)
   OR public_key_value = (?2)
