Feature: Get account info.
    Scenario: Get authenticated account v1.
        Given wirc is running on localhost
        When an authenticated user navigates to http://localhost:24816/api/v1/account
        Then the user should receive an account
        
    Scenario: Get unauthenticated account v1.
        Given wirc is running on localhost
        When a user navigates to http://localhost:24816/api/v1/account/testaccount
        Then the user should receive a generic account
