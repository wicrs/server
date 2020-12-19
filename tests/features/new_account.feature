Feature: Create an account

  Scenario: A user wants to create a new account
    Given wirc is running on localhost
    When an authenticated user attempts to create a new account
    Then the user should receive account information