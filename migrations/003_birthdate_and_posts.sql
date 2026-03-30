-- Replace age_group with birth_date
ALTER TABLE users ADD COLUMN IF NOT EXISTS birth_date DATE;
ALTER TABLE users DROP COLUMN IF EXISTS age_group;

-- Update sample users with birth dates
UPDATE users SET birth_date = '1996-03-15' WHERE email = 'admin@zet.kr';
UPDATE users SET birth_date = '2001-07-22' WHERE email = 'user1@test.kr';
UPDATE users SET birth_date = '2003-11-08' WHERE email = 'user2@test.kr';
UPDATE users SET birth_date = '1993-05-30' WHERE email = 'user3@test.kr';
UPDATE users SET birth_date = '2009-01-14' WHERE email = 'user4@test.kr';

-- Helper function: birth_date -> age group string
CREATE OR REPLACE FUNCTION age_group_from_birth(bd DATE) RETURNS TEXT AS $$
DECLARE
    age INT := EXTRACT(YEAR FROM age(bd));
BEGIN
    IF age < 20 THEN RETURN '10대';
    ELSIF age < 30 THEN RETURN '20대';
    ELSIF age < 40 THEN RETURN '30대';
    ELSIF age < 50 THEN RETURN '40대';
    ELSE RETURN '50대 이상';
    END IF;
END;
$$ LANGUAGE plpgsql IMMUTABLE;
