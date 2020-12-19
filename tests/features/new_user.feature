Feature: Get account info.
    Scenario: A user wants to create a new messaging identity associated with their account
        Given wirc is running on localhost
        When an authenticated user sends a name to http://localhost:24816/api/v1/account/adduser
        Then the user should receive a user
