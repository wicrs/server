Feature: Get user information

  Scenario: A user wants to view their private information
    Given wirc is running on localhost
    When an authenticated user navigates to http://localhost:24816/api/v1/account
    Then the user should receive user information

  Scenario: A user wants to see another user's public information
    Given wirc is running on localhost
    When a user navigates to http://localhost:24816/api/v1/account/testaccount
    Then the user should receive generic user information
