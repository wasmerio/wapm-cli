INSERT INTO wapm_public_keys 
  (user_key,
  public_key_id,
  public_key_value,
  key_type_identifier,
  date_added) 
VALUES 
((SELECT id
  FROM wapm_users
  WHERE name = (?1)),
  (?2),
  (?3),
  (?4),
  (?5))
