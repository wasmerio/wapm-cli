SELECT content
FROM wasm_contracts
WHERE contract_name = (?1)
  AND version = (?2)
