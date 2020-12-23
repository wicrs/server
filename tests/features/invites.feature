Feature: Invites

    Scenario: A user wants to generate an invite to a hub
        Given the user is in a hub
        When the user requests an invite to the hub
        Then the user should receive an invite

    Scenario: A user wants to join a hub using an invite link
        Given the user has an invite
        When the user uses the invite
        Then the user should receive an ID
