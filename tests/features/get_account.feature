Feature: Get account info.
    Scenario: A user wants to view their private account info
        Given wirc is running on localhost
        When an authenticated user navigates to http://localhost:24816/api/v1/account
        Then the user should receive an account
        
    Scenario: A user wants to see the public information of an account
        Given wirc is running on localhost
        When a user navigates to http://localhost:24816/api/v1/account/testaccount
        Then the user should receive a generic account
