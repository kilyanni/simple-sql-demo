-- Schema for Legally-Distinct-From-Moodle. Table groups follow the ER diagram
-- in the report, which also explains each index.
--
-- Deletes: what belongs to a course goes with it. A delete that would leave
-- surviving rows meaning something else is refused instead: accounts that
-- submitted, graded or posted, and questions, options, groups and groupings
-- that have already been used. The latter are deferred, because dropping the
-- whole course removes both sides and must still succeed.

CREATE TABLE "user" (
    uuid UUID PRIMARY KEY,
    email VARCHAR NOT NULL UNIQUE,
    username VARCHAR NOT NULL UNIQUE,
    external_id VARCHAR UNIQUE,  -- ID in external SSO auth system
    password_hash TEXT,  -- argon2id, for authenticating guests
    full_name VARCHAR NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- ═════════════════════════════════════════════
-- Kurse
-- ═════════════════════════════════════════════

CREATE TABLE course (
    uuid UUID PRIMARY KEY,
    title VARCHAR NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE,
    description TEXT,
    enrollment_key VARCHAR,  -- null = open enrollment
    max_capacity INTEGER,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE course_member (
    user_uuid UUID REFERENCES "user" (uuid) ON DELETE CASCADE,
    course_uuid UUID REFERENCES course (uuid) ON DELETE CASCADE,
    joined_at TIMESTAMP NOT NULL DEFAULT NOW(),
    status VARCHAR NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    timestart TIMESTAMP,
    timeend TIMESTAMP,
    PRIMARY KEY (user_uuid, course_uuid)
);

CREATE TABLE course_section (
    uuid UUID PRIMARY KEY,
    course_uuid UUID NOT NULL REFERENCES course (uuid) ON DELETE CASCADE,
    title VARCHAR NOT NULL,
    body TEXT,
    position INTEGER NOT NULL,
    visibility VARCHAR NOT NULL DEFAULT 'visible' CHECK (visibility IN ('visible', 'hidden'))
);

CREATE TABLE hypertext (
    uuid UUID PRIMARY KEY,
    section_uuid UUID NOT NULL REFERENCES course_section (uuid) ON DELETE CASCADE,
    title VARCHAR NOT NULL,
    body TEXT NOT NULL
);

CREATE TABLE file (
    uuid UUID PRIMARY KEY,
    section_uuid UUID NOT NULL REFERENCES course_section (uuid) ON DELETE CASCADE,
    title VARCHAR NOT NULL,
    type VARCHAR NOT NULL CHECK (type IN ('link', 'document', 'video')),
    url TEXT NOT NULL
);

-- ═════════════════════════════════════════════
-- RBAC
-- ═════════════════════════════════════════════

CREATE TABLE role (
    uuid UUID PRIMARY KEY,
    name VARCHAR NOT NULL UNIQUE,
    description TEXT
);

CREATE TABLE permission (
    uuid UUID PRIMARY KEY,
    name VARCHAR NOT NULL UNIQUE
);

CREATE TABLE role_permission (
    role_uuid UUID REFERENCES role (uuid) ON DELETE CASCADE,
    permission_uuid UUID REFERENCES permission (uuid) ON DELETE CASCADE,
    PRIMARY KEY (role_uuid, permission_uuid)
);

CREATE TABLE role_assignment (
    user_uuid UUID NOT NULL REFERENCES "user" (uuid) ON DELETE CASCADE,
    role_uuid UUID NOT NULL REFERENCES role (uuid) ON DELETE RESTRICT,
    course_uuid UUID NOT NULL REFERENCES course (uuid) ON DELETE CASCADE,
    PRIMARY KEY (user_uuid, role_uuid, course_uuid)
);

-- ═════════════════════════════════════════════
-- Gruppen & Gruppierungen
-- ═════════════════════════════════════════════

CREATE TABLE course_group (
    uuid UUID PRIMARY KEY,
    course_uuid UUID NOT NULL REFERENCES course (uuid) ON DELETE CASCADE,
    name VARCHAR NOT NULL,
    enrollment_key VARCHAR,
    UNIQUE (course_uuid, enrollment_key)
);

CREATE TABLE group_member (
    group_uuid UUID REFERENCES course_group (uuid) ON DELETE CASCADE,
    user_uuid UUID REFERENCES "user" (uuid) ON DELETE CASCADE,
    PRIMARY KEY (group_uuid, user_uuid)
);

CREATE TABLE grouping (
    uuid UUID PRIMARY KEY,
    course_uuid UUID NOT NULL REFERENCES course (uuid) ON DELETE CASCADE,
    name VARCHAR NOT NULL,
    description TEXT
);

CREATE TABLE grouping_group (
    grouping_uuid UUID REFERENCES grouping (uuid) ON DELETE CASCADE,
    group_uuid UUID REFERENCES course_group (uuid) ON DELETE CASCADE,
    PRIMARY KEY (grouping_uuid, group_uuid)
);

-- ═════════════════════════════════════════════
-- Aufgaben & Bewertung
-- ═════════════════════════════════════════════

CREATE TABLE task (
    uuid UUID PRIMARY KEY,
    course_section_uuid UUID NOT NULL REFERENCES course_section (uuid) ON DELETE CASCADE,
    name VARCHAR NOT NULL
);

CREATE TABLE file_upload (
    uuid UUID PRIMARY KEY,
    task_uuid UUID NOT NULL UNIQUE REFERENCES task (uuid) ON DELETE CASCADE,
    title VARCHAR NOT NULL,
    -- A quiz has no counterpart: its maximum is the sum of its questions.
    max_points NUMERIC(6, 2) NOT NULL,
    description TEXT,
    due_date TIMESTAMP,
    cut_off_date TIMESTAMP,
    group_mode BOOLEAN NOT NULL DEFAULT FALSE,
    -- null = any course group
    grouping_uuid UUID REFERENCES grouping (uuid) DEFERRABLE INITIALLY DEFERRED,
    blind_marking BOOLEAN NOT NULL DEFAULT FALSE,
    max_files INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE submission (
    uuid UUID PRIMARY KEY,
    file_upload_uuid UUID NOT NULL REFERENCES file_upload (uuid) ON DELETE CASCADE,
    submitter_uuid UUID NOT NULL REFERENCES "user" (uuid) ON DELETE RESTRICT,
    group_uuid UUID REFERENCES course_group (uuid) DEFERRABLE INITIALLY DEFERRED,
    file_refs TEXT[],  -- array of external blob refs/URLs
    comment TEXT,
    submitted_at TIMESTAMP NOT NULL DEFAULT NOW(),
    attempt_number INTEGER NOT NULL DEFAULT 1
);

-- A grade marks either a submission or a single free-text answer. MATCH FULL
-- keeps the answer key whole, the CHECK keeps exactly one of the two set.
CREATE TABLE grade (
    uuid UUID PRIMARY KEY,
    grader_uuid UUID NOT NULL REFERENCES "user" (uuid) ON DELETE RESTRICT,
    submission_uuid UUID UNIQUE REFERENCES submission (uuid) ON DELETE CASCADE,
    attempt_uuid UUID,
    question_uuid UUID,
    points NUMERIC(6, 2) NOT NULL,
    feedback TEXT,
    graded_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (attempt_uuid, question_uuid),
    CHECK ((submission_uuid IS NULL) != (attempt_uuid IS NULL))
);

-- ═════════════════════════════════════════════
-- Fragenbank & Quiz
-- ═════════════════════════════════════════════

CREATE TABLE question (
    uuid UUID PRIMARY KEY,
    course_uuid UUID NOT NULL REFERENCES course (uuid) ON DELETE CASCADE,
    body TEXT NOT NULL,
    type VARCHAR NOT NULL CHECK (type IN ('mcq', 'free_text')),
    default_points NUMERIC(6, 2) NOT NULL
);

CREATE TABLE question_option (
    uuid UUID PRIMARY KEY,
    question_uuid UUID NOT NULL REFERENCES question (uuid) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    body TEXT NOT NULL,
    fraction NUMERIC(3, 2) NOT NULL CHECK (fraction BETWEEN -1 AND 1),
    UNIQUE (question_uuid, position)
);

CREATE TABLE quiz (
    uuid UUID PRIMARY KEY,
    task_uuid UUID NOT NULL UNIQUE REFERENCES task (uuid) ON DELETE CASCADE,
    title VARCHAR NOT NULL,
    time_limit INTEGER,  -- minutes, null = unlimited
    available_from TIMESTAMP,
    available_until TIMESTAMP,
    attempts_allowed INTEGER,
    grading_method VARCHAR NOT NULL DEFAULT 'highest'
    CHECK (grading_method IN ('highest', 'average', 'first', 'last'))
);

CREATE TABLE quiz_question (
    quiz_uuid UUID REFERENCES quiz (uuid) ON DELETE CASCADE,
    question_uuid UUID REFERENCES question (uuid) DEFERRABLE INITIALLY DEFERRED,
    position INTEGER NOT NULL,
    points NUMERIC(6, 2) NOT NULL,
    PRIMARY KEY (quiz_uuid, question_uuid)
);

CREATE TABLE quiz_attempt (
    uuid UUID PRIMARY KEY,
    quiz_uuid UUID NOT NULL REFERENCES quiz (uuid) ON DELETE CASCADE,
    student_uuid UUID NOT NULL REFERENCES "user" (uuid) ON DELETE RESTRICT,
    started_at TIMESTAMP NOT NULL DEFAULT NOW(),
    submitted_at TIMESTAMP
);

CREATE TABLE quiz_answer_text (
    attempt_uuid UUID REFERENCES quiz_attempt (uuid) ON DELETE CASCADE,
    question_uuid UUID REFERENCES question (uuid) DEFERRABLE INITIALLY DEFERRED,
    free_text TEXT NOT NULL,
    PRIMARY KEY (attempt_uuid, question_uuid)
);

-- grade sits with the assignments, so its answer arc is only resolvable here.
ALTER TABLE grade ADD FOREIGN KEY (attempt_uuid, question_uuid)
REFERENCES quiz_answer_text (attempt_uuid, question_uuid) MATCH FULL ON DELETE CASCADE;

CREATE TABLE quiz_answer_option (
    attempt_uuid UUID REFERENCES quiz_attempt (uuid) ON DELETE CASCADE,
    selected_option_uuid UUID REFERENCES question_option (uuid) DEFERRABLE INITIALLY DEFERRED,
    PRIMARY KEY (attempt_uuid, selected_option_uuid)
);

-- An answer may only concern a question the attempted quiz actually asks.
-- Carrying quiz_uuid in the answer tables would express this as a foreign key,
-- but it is functionally dependent on attempt_uuid alone and would break 2NF.
CREATE FUNCTION answered_question_is_in_quiz() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM quiz_attempt AS qa
        INNER JOIN quiz_question AS qq ON qa.quiz_uuid = qq.quiz_uuid
        WHERE qa.uuid = NEW.attempt_uuid AND qq.question_uuid = NEW.question_uuid
    ) THEN
        RAISE EXCEPTION 'question % is not part of the quiz attempted in %',
        NEW.question_uuid, NEW.attempt_uuid;
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION selected_option_is_in_quiz() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM quiz_attempt AS qa
        INNER JOIN quiz_question AS qq ON qa.quiz_uuid = qq.quiz_uuid
        INNER JOIN question_option AS qo ON qq.question_uuid = qo.question_uuid
        WHERE qa.uuid = NEW.attempt_uuid AND qo.uuid = NEW.selected_option_uuid
    ) THEN
        RAISE EXCEPTION 'option % is not part of the quiz attempted in %',
        NEW.selected_option_uuid, NEW.attempt_uuid;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER quiz_answer_text_in_quiz
BEFORE INSERT OR UPDATE ON quiz_answer_text
FOR EACH ROW EXECUTE FUNCTION answered_question_is_in_quiz();

CREATE TRIGGER quiz_answer_option_in_quiz
BEFORE INSERT OR UPDATE ON quiz_answer_option
FOR EACH ROW EXECUTE FUNCTION selected_option_is_in_quiz();

-- ═════════════════════════════════════════════
-- Forum
-- ═════════════════════════════════════════════

CREATE TABLE forum (
    uuid UUID PRIMARY KEY,
    course_section_uuid UUID NOT NULL REFERENCES course_section (uuid) ON DELETE CASCADE,
    title VARCHAR NOT NULL,
    description TEXT,
    type VARCHAR NOT NULL DEFAULT 'general' CHECK (type IN ('general', 'news', 'qanda'))
);

CREATE TABLE discussion (
    uuid UUID PRIMARY KEY,
    forum_uuid UUID NOT NULL REFERENCES forum (uuid) ON DELETE CASCADE,
    root_post_uuid UUID NOT NULL UNIQUE,
    title VARCHAR NOT NULL,
    pinned BOOLEAN NOT NULL DEFAULT FALSE,
    locked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE post (
    uuid UUID PRIMARY KEY,
    discussion_uuid UUID NOT NULL REFERENCES discussion (uuid) ON DELETE CASCADE,
    -- null = top-level
    parent_uuid UUID REFERENCES post (uuid) ON DELETE CASCADE,
    author_uuid UUID NOT NULL REFERENCES "user" (uuid) ON DELETE RESTRICT,
    body TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (uuid, discussion_uuid)
);

-- A discussion and its root post reference each other; the deferred check lets
-- both be inserted in one transaction, and a discussion without a post is impossible.
ALTER TABLE discussion ADD FOREIGN KEY (root_post_uuid, uuid)
REFERENCES post (uuid, discussion_uuid) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE notification (
    uuid UUID PRIMARY KEY,
    recipient_uuid UUID NOT NULL REFERENCES "user" (uuid) ON DELETE CASCADE,
    type VARCHAR NOT NULL,
    payload JSONB,
    read_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- ═════════════════════════════════════════════
-- View
-- ═════════════════════════════════════════════

CREATE VIEW v_course_overview AS
SELECT
    c.uuid,
    c.title AS course,
    c.start_date,
    c.end_date,
    COUNT(DISTINCT cm.user_uuid) FILTER (WHERE cm.status = 'active') AS active_members,
    COUNT(DISTINCT cs.uuid) AS sections,
    COUNT(DISTINCT f.uuid) AS files,
    COUNT(DISTINCT fu.uuid) AS assignments,
    COUNT(DISTINCT q.uuid) AS quizzes,
    COUNT(DISTINCT fo.uuid) AS forums
FROM course AS c
LEFT JOIN course_member AS cm ON c.uuid = cm.course_uuid
LEFT JOIN course_section AS cs ON c.uuid = cs.course_uuid
LEFT JOIN file AS f ON cs.uuid = f.section_uuid
LEFT JOIN task AS t ON cs.uuid = t.course_section_uuid
LEFT JOIN file_upload AS fu ON t.uuid = fu.task_uuid
LEFT JOIN quiz AS q ON t.uuid = q.task_uuid
LEFT JOIN forum AS fo ON cs.uuid = fo.course_section_uuid
GROUP BY c.uuid;

-- Course members holding the standard student role.
CREATE VIEW v_course_student AS
SELECT
    cm.course_uuid,
    cm.user_uuid,
    cm.status
FROM course_member AS cm
INNER JOIN role_assignment AS ra ON cm.user_uuid = ra.user_uuid AND cm.course_uuid = ra.course_uuid
INNER JOIN role AS r ON ra.role_uuid = r.uuid
WHERE r.name = 'Studierende';

-- Users a submission counts for: the submitter, or for group assignments every
-- member of the submitting group, provided the group belongs to the course and,
-- if the assignment is restricted to a grouping, to that grouping.
CREATE VIEW v_submission_member AS
SELECT
    s.uuid AS submission_uuid,
    s.file_upload_uuid,
    s.submitter_uuid AS user_uuid
FROM submission AS s
INNER JOIN file_upload AS fu ON s.file_upload_uuid = fu.uuid
WHERE NOT fu.group_mode
UNION ALL
SELECT
    s.uuid,
    s.file_upload_uuid,
    gm.user_uuid
FROM submission AS s
INNER JOIN file_upload AS fu ON s.file_upload_uuid = fu.uuid
INNER JOIN task AS t ON fu.task_uuid = t.uuid
INNER JOIN course_section AS cs ON t.course_section_uuid = cs.uuid
INNER JOIN course_group AS cg ON s.group_uuid = cg.uuid AND cs.course_uuid = cg.course_uuid
INNER JOIN group_member AS gm ON cg.uuid = gm.group_uuid
WHERE
    fu.group_mode
    AND (fu.grouping_uuid IS NULL OR EXISTS (
        SELECT 1 FROM grouping_group AS gg
        WHERE gg.grouping_uuid = fu.grouping_uuid AND gg.group_uuid = cg.uuid
    ));

-- Points of a submitted attempt: selection questions score themselves from
-- question_option.fraction, free text scores what a marker awarded it in grade.
-- An unmarked free-text answer scores zero, like an unanswered question.
CREATE VIEW v_quiz_attempt_points AS
WITH chosen AS (
    SELECT
        qao.attempt_uuid,
        SUM(qq.points * qo.fraction) AS points
    FROM quiz_answer_option AS qao
    INNER JOIN quiz_attempt AS qa2 ON qao.attempt_uuid = qa2.uuid
    INNER JOIN question_option AS qo ON qao.selected_option_uuid = qo.uuid
    INNER JOIN quiz_question AS qq
        ON qa2.quiz_uuid = qq.quiz_uuid AND qo.question_uuid = qq.question_uuid
    GROUP BY qao.attempt_uuid
),

written AS (
    SELECT
        attempt_uuid,
        SUM(points) AS points
    FROM grade
    WHERE attempt_uuid IS NOT NULL
    GROUP BY attempt_uuid
)

SELECT
    qa.uuid AS attempt_uuid,
    qa.quiz_uuid,
    qa.student_uuid,
    qa.started_at,
    COALESCE(chosen.points, 0) + COALESCE(written.points, 0) AS points
FROM quiz_attempt AS qa
LEFT JOIN chosen ON qa.uuid = chosen.attempt_uuid
LEFT JOIN written ON qa.uuid = written.attempt_uuid
WHERE qa.submitted_at IS NOT NULL;

CREATE VIEW v_quiz_student_points AS
SELECT
    ap.quiz_uuid,
    ap.student_uuid,
    COUNT(*) AS attempts,
    CASE q.grading_method
        WHEN 'highest' THEN MAX(ap.points)
        WHEN 'average' THEN AVG(ap.points)
        WHEN 'first' THEN (ARRAY_AGG(ap.points ORDER BY ap.started_at, ap.attempt_uuid))[1]
        WHEN 'last' THEN (ARRAY_AGG(ap.points ORDER BY ap.started_at DESC, ap.attempt_uuid ASC))[1]
    END AS points
FROM v_quiz_attempt_points AS ap
INNER JOIN quiz AS q ON ap.quiz_uuid = q.uuid
GROUP BY ap.quiz_uuid, ap.student_uuid, q.grading_method;

-- ═════════════════════════════════════════════
-- Indexes
-- ═════════════════════════════════════════════

CREATE INDEX idx_course_member_course_uuid ON course_member (course_uuid);
CREATE INDEX idx_submission_upload_time ON submission (file_upload_uuid, submitted_at);
CREATE INDEX idx_quiz_attempt_submitted ON quiz_attempt (quiz_uuid) WHERE submitted_at IS NOT NULL;
CREATE INDEX idx_quiz_attempt_student ON quiz_attempt (student_uuid, started_at DESC);
CREATE INDEX idx_quiz_answer_option_option ON quiz_answer_option (selected_option_uuid);
CREATE INDEX idx_notification_unread ON notification (recipient_uuid) WHERE read_at IS NULL;
CREATE INDEX idx_post_discussion_uuid ON post (discussion_uuid);
CREATE UNIQUE INDEX uq_discussion_root_post ON post (discussion_uuid) WHERE parent_uuid IS NULL;
-- An attempt is counted per person, or per group where the assignment is one.
CREATE UNIQUE INDEX uq_submission_attempt
ON submission (file_upload_uuid, submitter_uuid, attempt_number) WHERE group_uuid IS NULL;
CREATE UNIQUE INDEX uq_group_submission_attempt
ON submission (file_upload_uuid, group_uuid, attempt_number) WHERE group_uuid IS NOT NULL;
