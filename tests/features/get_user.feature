Feature: Get user information

  Scenario: A user wants to view their private information
    Given wirc is running on localhost
    When the authenticated user requests their information
    Then the user should receive user information

  Scenario: A user wants to see another user's public information
    Given wirc is running on localhost
    When a user requests another user's information
    Then the user should receive basic user information
