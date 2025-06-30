-- BASIC USAGE OF THIS TABLE:
-- use to create dns record MySQL table.
-- name is for block of FQDN that this DNS Server is supposed to resolve, say
-- page.super.example.com, if this DNS Server is example.com, then name is super.
-- since super is hosting page, super will require a dns_ip AND dns_port. if
-- super.example.com is a valid viewable webpage, then super will also need fields
-- domain_ip and domain_port filled in.

-- IF THIS DNS HOST NEEDS TO BE MOVED:
-- set the . record (record where name=".") dns_ip and dns_port to that of the new
-- DNS server. The code will handle the redirect for you.

-- IF YOU WISH TO SET UP WILDCARD REDIRECTION:
-- say nothing exists at nothing.example.com, but you want the DNS Server to redirect
-- the client somewhere, fill in the . record with domain_ip and domain_port. This functionality
-- is especially useful if you are say hosting the DNS Server for .com, and you want that if a
-- client requests a nonexistant .com domain to be taken to your 'purchase this domain' page.

CREATE TABLE dns_records (
  id INT AUTO_INCREMENT PRIMARY KEY,
  name VARCHAR(255) UNIQUE NOT NULL,
  domain_ip VARCHAR(63) NULL,
  domain_port SMALLINT UNSIGNED NULL CHECK (domain_port BETWEEN 0 AND 25565),
  dns_ip VARCHAR(63) NULL,
  dns_port SMALLINT UNSIGNED NULL CHECK (dns_port BETWEEN 0 AND 25565)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;