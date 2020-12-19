Feature: Create guilds

  Scenario: A user wants to create a guild
    Given wirc is running on localhost
    When an authenticated user attempts to create a new guild
    Then the server should respond with the OK status