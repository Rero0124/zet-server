-- Drop companies table and posts.company_id (role=business suffices)
ALTER TABLE posts DROP COLUMN company_id;
DROP TABLE companies;
