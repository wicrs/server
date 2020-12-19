Feature: Invalidate authentication tokens.
    Scenario: A user wants to invalidate all authentication tokens associated with their account
        Given wirc is running on localhost
        When an authenticated user navigates to http://localhost:24816/api/v1/invalidate
        Then the user should receive text Success!
        When an authenticated user navigates to http://localhost:24816/api/v1/account
        Then the user should receive text Invalid authentication details.
