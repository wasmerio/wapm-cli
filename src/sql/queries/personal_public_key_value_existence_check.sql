SELECT public_key_tag
FROM personal_keys
WHERE public_key_tag = (?1)
   OR public_key_value = (?2)
