Feature: Get a link the correct OAuth page.
    Scenario: A user wants to login using GitHub
        Given wirc is running on localhost
        When a user navigates to http://localhost:24816/api/v1/login/github
        Then the user should be redirected to https://github.com/login
