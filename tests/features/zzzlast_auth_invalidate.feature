Feature: ZZZ Invalidate a user's authentication tokens

    Scenario: ZZZ An authenticated user wants to invalidate all authentication tokens they have created
        Given the user has an account
        When the user tells the server to invalidate all of their tokens
        And the user requests their information
        Then the user should be denied access
