Feature: Invalidate authentication tokens.
    Scenario: Invalidate tokens for account v1.
        Given wirc is running on localhost
        When an authenticated user navigates to http://localhost:24816/api/v1/invalidate
        Then the user should receive text Success!
        When an authenticated user navigates to http://localhost:24816/api/v1/account
        Then the user should receive text Invalid authentication details.
