<a href={somethingUnsafe}>unsafe link</a>
<iframe src="javascript:alert('bad')"/>

location.href = unsafeValueFromUser;
