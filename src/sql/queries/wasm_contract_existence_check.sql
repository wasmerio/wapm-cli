SELECT 1
FROM wasm_contracts
WHERE contract_name = (?1) 
   AND version = (?2)
LIMIT 1
