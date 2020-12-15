Feature: Get account info.
    Scenario: Create new user for account v1.
        Given wirc is running on localhost
        When an authenticated user sends a name to http://localhost:24816/api/v1/account/adduser
        Then the user should receive a user
