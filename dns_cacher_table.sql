-- Same as dns_lookuo_table.sql, however this one stores FQDNs in the name field.
-- The "." record is used for if the DNS Cacher saver has moved, it specifies the location.

-- As this is meant for FQDNs and not FQDN blocks, the dns fields have been removed,
-- and thus there are only domain_ip and domain_port.

-- To avoid performance issues, the cacher can only cache FQDNs shorter than or equal to
-- 255 characters.

CREATE TABLE dns_cache (
  id INT AUTO_INCREMENT PRIMARY KEY,
  name VARCHAR(255) UNIQUE NOT NULL,
  domain_ip VARCHAR(63) NULL,
  domain_port SMALLINT UNSIGNED NULL CHECK (domain_port BETWEEN 0 AND 25565),
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;