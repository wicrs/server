Feature: Get a link the correct authentication service

  Scenario: A user wants to login using GitHub
    Given wirc is running on localhost
    When a user attempts to login using their GitHub account
    Then the user should be redirected to the github login page
