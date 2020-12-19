Feature: Invalidate a user's authentication tokens

  Scenario: A user wants to invalidate all authentication tokens they have created
    Given wirc is running on localhost
    When an authenticated user tells the server to invalidate all of their tokens
    When the user attempts to view their private information
    Then the user should be denied access
